// Copyright 2026 the Velato Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::Composition;
use super::model::{Content, Layer, LayerReference, Shape, TransformComponents};
use kurbo::{Affine, PathEl, Point};

/// Runtime layer-array indices from the root through nested precomp instances.
/// Valid only for the composition layout that produced them, not authored Lottie IDs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OccurrencePath(pub Vec<usize>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvaluationError {
    NonFiniteTime,
    InvalidStretch(OccurrencePath),
    InvalidTransform(OccurrencePath),
    InvalidParent(OccurrencePath),
    UnresolvedParent { path: OccurrencePath, id: usize },
    ParentCycle(OccurrencePath),
    InvalidMatte(OccurrencePath),
    UnresolvedMatte { path: OccurrencePath, id: usize },
    MatteCycle(OccurrencePath),
    MissingAsset(String),
    PrecompCycle(String),
}

impl std::fmt::Display for EvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnresolvedParent { path, id } => write!(
                f,
                "No parent layer with ind {id} in the composition containing {path:?}"
            ),
            Self::UnresolvedMatte { path, id } => write!(
                f,
                "No matte layer with ind {id} in the composition containing {path:?}"
            ),
            _ => write!(f, "Lottie evaluation failed: {self:?}"),
        }
    }
}
impl std::error::Error for EvaluationError {}

/// Drawing-independent layer evaluation, including hidden and out-of-range layers.
/// Coordinates are Lottie pixels, with positive Y pointing down.
#[derive(Debug)]
pub struct EvaluatedLayer<'a> {
    pub path: OccurrencePath,
    pub layer: &'a Layer,
    /// Validated transform-parent index in the containing composition.
    pub parent: Option<usize>,
    /// Matte blend mode and validated source index in the containing composition.
    pub matte: Option<(peniko::BlendMode, usize)>,
    /// Frame in this occurrence's containing composition.
    pub frame: f64,
    /// Property sampling frame in the containing composition, before child remapping.
    pub property_frame: f64,
    pub in_range: bool,
    /// Normal-tree visibility, including containing precomp visibility.
    pub visible: bool,
    pub authored_components: Option<TransformComponents>,
    /// Maps layer content into its transform parent's content space, or the
    /// containing composition when there is no transform parent.
    pub local_transform: Affine,
    /// Maps layer content to root composition pixels, excluding output transforms.
    pub full_transform: Affine,
    /// Layer opacity as a fraction, without transform-parent opacity.
    pub opacity: f64,
    pub children: Vec<EvaluatedLayer<'a>>,
}

impl EvaluatedLayer<'_> {
    /// Evaluated anchor in root composition pixels, if authored components exist.
    pub fn anchor(&self) -> Option<Point> {
        self.authored_components
            .map(|c| self.full_transform * c.anchor)
    }

    /// Authored paths BEFORE modifiers, repeaters, masks, paint, stroke expansion,
    /// and effects. Elements are group-local; transforms map to root composition
    /// pixels. These are not rendered silhouettes.
    pub fn authored_paths(&self) -> Vec<AuthoredPath> {
        fn collect(
            shapes: &[Shape],
            frame: f64,
            transform: Affine,
            indices: &mut Vec<usize>,
            paths: &mut Vec<AuthoredPath>,
        ) {
            for (index, shape) in shapes.iter().enumerate() {
                indices.push(index);
                match shape {
                    Shape::Geometry(geometry) => {
                        let mut elements = Vec::new();
                        geometry.evaluate(frame, &mut elements);
                        paths.push(AuthoredPath {
                            indices: indices.clone(),
                            elements,
                            transform,
                        });
                    }
                    Shape::Group(children, group) => {
                        let local = group.as_ref().map_or(Affine::IDENTITY, |g| {
                            g.transform.evaluate(frame).into_owned()
                        });
                        collect(children, frame, transform * local, indices, paths);
                    }
                    _ => {}
                }
                indices.pop();
            }
        }
        let mut paths = Vec::new();
        if let Content::Shape(shapes) = &self.layer.content {
            collect(
                shapes,
                self.property_frame,
                self.full_transform,
                &mut Vec::new(),
                &mut paths,
            );
        }
        paths
    }
}

#[derive(Clone, Debug)]
pub struct AuthoredPath {
    /// Runtime indices through nested shape groups, not authored shape identifiers.
    pub indices: Vec<usize>,
    /// Pre-modifier elements in the owning group's coordinates, or layer coordinates
    /// for ungrouped geometry.
    pub elements: Vec<PathEl>,
    /// Maps group coordinates to root composition pixels, excluding output transforms.
    pub transform: Affine,
}

#[derive(Debug)]
pub struct EvaluatedComposition<'a> {
    pub layers: Vec<EvaluatedLayer<'a>>,
}

impl EvaluatedComposition<'_> {
    pub fn get(&self, path: &OccurrencePath) -> Option<&EvaluatedLayer<'_>> {
        let mut layers = &self.layers;
        let mut result = None;
        for index in &path.0 {
            let layer = layers.get(*index)?;
            result = Some(layer);
            layers = &layer.children;
        }
        result
    }
}

impl Composition {
    /// Evaluates all occurrences, including reference-only content, without drawing.
    /// Invalid references, cycles, non-finite timing and transforms are errors.
    /// Precomp occurrence expansion is unbounded; validate untrusted input separately.
    pub fn evaluate(&self, frame: f64) -> Result<EvaluatedComposition<'_>, EvaluationError> {
        let layers = evaluate_layers(
            self,
            &self.layers,
            frame,
            Affine::IDENTITY,
            true,
            &mut Vec::new(),
            &mut Vec::new(),
        )?;
        Ok(EvaluatedComposition { layers })
    }
}

#[derive(Clone, Copy)]
enum VisitState {
    Unvisited,
    Visiting,
    Visited,
}

/// Dependencies must be validated indices. The explicit stack avoids recursive
/// traversal of long parent or matte chains during validation.
fn dependency_order(
    count: usize,
    dependency: impl Fn(usize) -> Option<usize>,
) -> Result<Vec<usize>, usize> {
    let mut states = vec![VisitState::Unvisited; count];
    let mut stack = Vec::new();
    let mut order = Vec::with_capacity(count);
    for start in 0..count {
        let mut current = Some(start);
        while let Some(index) = current {
            match states[index] {
                VisitState::Visited => break,
                VisitState::Visiting => return Err(index),
                VisitState::Unvisited => {}
            }
            states[index] = VisitState::Visiting;
            stack.push(index);
            current = dependency(index);
        }
        while let Some(index) = stack.pop() {
            states[index] = VisitState::Visited;
            order.push(index);
        }
    }
    Ok(order)
}

fn evaluate_layers<'a>(
    animation: &'a Composition,
    layers: &'a [Layer],
    frame: f64,
    outer: Affine,
    visible: bool,
    prefix: &mut Vec<usize>,
    assets: &mut Vec<String>,
) -> Result<Vec<EvaluatedLayer<'a>>, EvaluationError> {
    if !frame.is_finite() {
        return Err(EvaluationError::NonFiniteTime);
    }
    let mut result = Vec::with_capacity(layers.len());
    for (index, layer) in layers.iter().enumerate() {
        let mut indices = prefix.clone();
        indices.push(index);
        let path = OccurrencePath(indices);
        let parent = match layer.parent {
            Some(LayerReference::Resolved(index)) => {
                if index >= layers.len() {
                    return Err(EvaluationError::InvalidParent(path));
                }
                Some(index)
            }
            Some(LayerReference::Unresolved(id)) => {
                return Err(EvaluationError::UnresolvedParent { path, id });
            }
            None => None,
        };
        let matte = match layer.mask_layer {
            Some((mode, LayerReference::Resolved(index))) => {
                if index >= layers.len() {
                    return Err(EvaluationError::InvalidMatte(path));
                }
                Some((mode, index))
            }
            Some((_, LayerReference::Unresolved(id))) => {
                return Err(EvaluationError::UnresolvedMatte { path, id });
            }
            None => None,
        };
        if matches!(layer.content, Content::Instance { .. })
            && (!layer.stretch.is_finite() || layer.stretch == 0.0)
        {
            return Err(EvaluationError::InvalidStretch(path));
        }
        let authored_components = layer.transform.components(frame);
        let local_transform = authored_components.map_or_else(
            || layer.transform.evaluate(frame).into_owned(),
            TransformComponents::matrix,
        );
        if !local_transform.is_finite() {
            return Err(EvaluationError::InvalidTransform(path));
        }
        let in_range = layer.frames.contains(&frame);
        result.push(EvaluatedLayer {
            path,
            layer,
            parent,
            matte,
            frame,
            property_frame: frame,
            in_range,
            visible: visible && in_range && !layer.hidden && !layer.is_mask,
            authored_components,
            local_transform,
            full_transform: local_transform,
            opacity: layer.opacity.evaluate(frame) / 100.0,
            children: Vec::new(),
        });
    }
    let parent_order = dependency_order(result.len(), |index| result[index].parent)
        .map_err(|index| EvaluationError::ParentCycle(result[index].path.clone()))?;
    for index in parent_order {
        let parent_transform = result[index]
            .parent
            .map_or(outer, |parent| result[parent].full_transform);
        let transform = parent_transform * result[index].local_transform;
        if !transform.is_finite() {
            return Err(EvaluationError::InvalidTransform(
                result[index].path.clone(),
            ));
        }
        result[index].full_transform = transform;
    }
    dependency_order(result.len(), |index| {
        result[index].matte.map(|(_, source)| source)
    })
    .map_err(|index| EvaluationError::MatteCycle(result[index].path.clone()))?;
    for node in &mut result {
        if let Content::Instance { name, time_remap } = &node.layer.content {
            // Preserve the existing tm sampling contract; only child content is remapped.
            let child_frame = if let Some(tm) = time_remap {
                let sample_frame = frame / node.layer.stretch - node.layer.start_frame;
                if !sample_frame.is_finite() {
                    return Err(EvaluationError::NonFiniteTime);
                }
                tm.evaluate(sample_frame) * animation.frame_rate
            } else {
                frame / node.layer.stretch - node.layer.start_frame / node.layer.stretch
            };
            if !child_frame.is_finite() {
                return Err(EvaluationError::NonFiniteTime);
            }
            if assets.contains(name) {
                return Err(EvaluationError::PrecompCycle(name.clone()));
            }
            let children = animation
                .assets
                .get(name)
                .ok_or_else(|| EvaluationError::MissingAsset(name.clone()))?;
            assets.push(name.clone());
            prefix.push(*node.path.0.last().unwrap());
            node.children = evaluate_layers(
                animation,
                children,
                child_frame,
                node.full_transform,
                node.visible,
                prefix,
                assets,
            )?;
            prefix.pop();
            assets.pop();
        } else if let Content::Image { asset_id } = &node.layer.content {
            if !animation.images.contains_key(asset_id) {
                return Err(EvaluationError::MissingAsset(asset_id.clone()));
            }
        }
    }
    Ok(result)
}
