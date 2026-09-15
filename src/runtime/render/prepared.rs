// Copyright 2026 the Velato Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use kurbo::Shape as KurboShape;

#[derive(Debug)]
pub struct PreparedScene {
    commands: Vec<Command>,
    elements: Vec<PathEl>,
    tolerance: f64,
    rejected: bool,
}

#[derive(Debug)]
enum Command {
    Layer(peniko::BlendMode, f32, Affine, Range<usize>),
    Clip(Affine, Range<usize>),
    Pop,
    Draw(Option<fixed::Stroke>, Affine, fixed::Brush, Range<usize>),
    Begin(String, usize),
    End,
    Emission(f32),
}

impl Renderer {
    pub fn try_prepare(
        &mut self,
        animation: &Composition,
        frame: f64,
        transform: Affine,
        alpha: f64,
        tolerance: f64,
    ) -> Result<Option<PreparedScene>, EvaluationError> {
        let mut scene = PreparedScene {
            commands: Vec::new(),
            elements: Vec::new(),
            tolerance,
            rejected: false,
        };
        self.try_append(animation, frame, transform, alpha, &mut scene)?;
        Ok((!scene.rejected).then_some(scene))
    }
}

impl PreparedScene {
    pub fn append(&self, sink: &mut impl RenderSink) {
        for command in &self.commands {
            match command {
                Command::Layer(blend, alpha, transform, range) => {
                    sink.push_layer(*blend, *alpha, *transform, &&self.elements[range.clone()])
                }
                Command::Clip(transform, range) => {
                    sink.push_clip_layer(*transform, &&self.elements[range.clone()])
                }
                Command::Pop => sink.pop_layer(),
                Command::Draw(stroke, transform, brush, range) => sink.draw(
                    stroke.as_ref(),
                    *transform,
                    brush,
                    &&self.elements[range.clone()],
                ),
                Command::Begin(name, index) => sink.begin_layer_group(name, *index),
                Command::End => sink.end_layer_group(),
                Command::Emission(value) => sink.set_layer_emission(*value),
            }
        }
    }

    fn push(&mut self, command: Command) {
        if self.commands.len() >= 4096 {
            self.rejected = true;
        }
        if !self.rejected {
            self.commands.push(command);
        }
    }

    fn path(&mut self, shape: &impl KurboShape) -> Range<usize> {
        let start = self.elements.len();
        if !self.rejected {
            for element in shape.path_elements(self.tolerance) {
                if self.elements.len() == 65536 {
                    self.rejected = true;
                    break;
                }
                self.elements.push(element);
            }
        }
        start..self.elements.len()
    }
}

impl RenderSink for PreparedScene {
    fn push_layer(
        &mut self,
        blend: impl Into<peniko::BlendMode>,
        alpha: f32,
        transform: Affine,
        shape: &impl KurboShape,
    ) {
        let path = self.path(shape);
        self.push(Command::Layer(blend.into(), alpha, transform, path));
    }
    fn push_clip_layer(&mut self, transform: Affine, shape: &impl KurboShape) {
        let path = self.path(shape);
        self.push(Command::Clip(transform, path));
    }
    fn pop_layer(&mut self) {
        self.push(Command::Pop);
    }
    fn draw(
        &mut self,
        stroke: Option<&fixed::Stroke>,
        transform: Affine,
        brush: &fixed::Brush,
        shape: &impl KurboShape,
    ) {
        let path = self.path(shape);
        if !self.rejected {
            self.push(Command::Draw(
                stroke.cloned(),
                transform,
                brush.clone(),
                path,
            ));
        }
    }
    fn draw_image(&mut self, _: &ImageAsset, _: Affine, _: f64) {
        self.rejected = true;
    }
    fn push_filter(&mut self, _: Affine, _: &FilterEffect) -> FilterLayerResult {
        self.rejected = true;
        FilterLayerResult::Unsupported
    }
    fn begin_layer_group(&mut self, name: &str, index: usize) {
        if name.len() > 256 {
            self.rejected = true;
        }
        if !self.rejected {
            self.push(Command::Begin(name.to_owned(), index));
        }
    }
    fn end_layer_group(&mut self) {
        self.push(Command::End);
    }
    fn set_layer_emission(&mut self, value: f32) {
        self.push(Command::Emission(value));
    }
}
