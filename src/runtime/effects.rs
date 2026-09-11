// Copyright 2026 the Velato Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::model::LayerEffect;
use kurbo::Vec2;
use peniko::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub enum FilterLayerResult {
    Pushed,
    Unsupported,
}

/// Sampling outside the source bounds for Gaussian blur.
///
/// Evaluation maps Gaussian blur `ef[2]` to `Wrap` for `1`, and `Duplicate`
/// otherwise, matching [lottie-web 5.13.0].
///
/// [lottie-web 5.13.0]: https://github.com/airbnb/lottie-web/blob/v5.13.0/player/js/elements/svgElements/effects/SVGGaussianBlurEffect.js
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlurEdgeMode {
    /// Extend the edge samples beyond the source bounds.
    Duplicate,
    /// Repeat the source across its bounds.
    Wrap,
}

/// A layer effect evaluated for a single frame and passed to
/// [`RenderSink::push_filter`](super::RenderSink::push_filter).
/// Spatial parameters are in layer coordinates; support depends on the sink.
#[derive(Clone, Debug)]
pub enum FilterEffect {
    GaussianBlur {
        /// Horizontal and vertical Gaussian sigma, not the authored blurriness value.
        std_deviation: Vec2,
        edge_mode: BlurEdgeMode,
    },
    DropShadow {
        /// Shadow color with effect opacity included in its alpha.
        color: Color,
        /// Shadow displacement in layer coordinates.
        offset: Vec2,
        /// Gaussian sigma, not the authored softness value.
        std_deviation: f64,
    },
    Fill {
        /// Fill color with effect opacity included in its alpha.
        color: Color,
    },
    Tint {
        /// Output color for black input.
        black: Color,
        /// Output color for white input.
        white: Color,
        /// Tint strength in the range `0.0..=1.0`.
        amount: f64,
    },
}

impl LayerEffect {
    pub fn evaluate(&self, frame: f64) -> Option<FilterEffect> {
        let nonnegative = |value: f64| {
            if value.is_finite() {
                value.max(0.0)
            } else {
                0.0
            }
        };
        Some(match self {
            Self::GaussianBlur {
                blurriness,
                dimensions,
                wrap,
            } => {
                // Match lottie-web's AE blurriness-to-sigma conversion.
                let sigma = nonnegative(blurriness.evaluate(frame)) * 0.3;
                if sigma == 0.0 {
                    return None;
                }
                let dimensions = dimensions.evaluate(frame) as u32;
                FilterEffect::GaussianBlur {
                    std_deviation: Vec2::new(
                        if dimensions == 3 { 0.0 } else { sigma },
                        if dimensions == 2 { 0.0 } else { sigma },
                    ),
                    // lottie-web maps 1 to wrap and all other values to duplicate, not transparent.
                    edge_mode: if wrap.evaluate(frame) == 1.0 {
                        BlurEdgeMode::Wrap
                    } else {
                        BlurEdgeMode::Duplicate
                    },
                }
            }
            Self::DropShadow {
                color,
                opacity,
                angle,
                distance,
                softness,
            } => {
                let opacity = (nonnegative(opacity.evaluate(frame)) / 255.0).min(1.0);
                if opacity == 0.0 {
                    return None;
                }
                let angle = angle.evaluate(frame).to_radians();
                let distance = distance.evaluate(frame);
                if !angle.is_finite() || !distance.is_finite() {
                    return None;
                }
                FilterEffect::DropShadow {
                    color: color
                        .evaluate_or(frame, Color::TRANSPARENT)
                        .multiply_alpha(opacity as f32),
                    offset: Vec2::new(angle.sin() * distance, -angle.cos() * distance),
                    // Drop Shadow uses a different AE softness scale from Gaussian Blur.
                    std_deviation: nonnegative(softness.evaluate(frame)) * 0.25,
                }
            }
            Self::Fill { color, opacity } => FilterEffect::Fill {
                color: color
                    .evaluate_or(frame, Color::TRANSPARENT)
                    .multiply_alpha(nonnegative(opacity.evaluate(frame)).min(1.0) as f32),
            },
            Self::Tint {
                black,
                white,
                amount,
            } => {
                let amount = (nonnegative(amount.evaluate(frame)) / 100.0).min(1.0);
                if amount == 0.0 {
                    return None;
                }
                FilterEffect::Tint {
                    black: black.evaluate_or(frame, Color::BLACK),
                    white: white.evaluate_or(frame, Color::WHITE),
                    amount,
                }
            }
        })
    }
}
