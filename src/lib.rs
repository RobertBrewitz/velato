// Copyright 2024 the Velato Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Parse and render Lottie animations through the renderer-independent [`RenderSink`] interface.
//!
//! Use the optional built-in Vello integration or implement [`RenderSink`] for your own renderer.
//! Rendering support depends on both Velato and the chosen sink; see the
//! [known limitations](crate#unsupported-features).
//!
//! With the `vello` feature enabled, this crate re-exports the compatible version of Vello.
//!
//! ## Usage
//!
//! ```no_run
//! # use std::str::FromStr;
//! use velato::vello;
//!
//! // Parse your lottie file
//! let lottie = include_str!("../examples/assets/google_fonts/Tiger.json");
//! let composition = velato::Composition::from_str(lottie).expect("valid file");
//!
//! // Render to a scene (requires the `vello` feature).
//! let mut renderer = velato::Renderer::new();
//! let frame = 0.0; // Arbitrary number chosen. Ensure it's a valid frame!
//! let transform = vello::kurbo::Affine::IDENTITY;
//! let alpha = 1.0;
//! let images = std::collections::HashMap::new();
//! let scene = renderer.render_to_vello_scene(&composition, &images, frame, transform, alpha);
//! ```
//!
//! # Unsupported features
//!
//! Known missing or incomplete features include:
//! - Position keyframe spatial interpolation (`ti`, `to`)
//! - Time remapping (`tm`)
//! - Text
//! - Built-in image loading and decoding, including embedded images (asset metadata is available
//!   to callers and sinks)
//! - Advanced shapes (stroke dash, zig-zag, etc.)
//! - Layer effects other than Gaussian blur, drop shadow, fill, and tint (these require sink support)
//! - Layer styles
//! - Accurate gradient opacity stops, including stops at positions without a corresponding color stop
//! - Split rotations
//! - Split positions in repeater transforms

// LINEBENDER LINT SET - lib.rs - v4
// See https://linebender.org/wiki/canonical-lints/
// These lints shouldn't apply to examples or tests.
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
// These lints shouldn't apply to examples.
#![warn(clippy::print_stdout, clippy::print_stderr)]
// Targeting e.g. 32-bit means structs containing usize can give false positives for 64-bit.
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]
// END LINEBENDER LINT SET
#![cfg_attr(docsrs, feature(doc_cfg))]
// The following lints are part of the Linebender standard set,
// but resolving them has been deferred for now.
// Feel free to send a PR that solves one or more of these.
#![allow(unused, reason = "Many lottie types we don't yet support.")]
#![allow(
    unreachable_pub,
    missing_docs,
    elided_lifetimes_in_paths,
    single_use_lifetimes,
    unused_qualifications,
    clippy::empty_docs,
    clippy::use_self,
    clippy::return_self_not_must_use,
    clippy::cast_possible_truncation,
    clippy::shadow_unrelated,
    clippy::missing_assert_message,
    clippy::missing_errors_doc,
    clippy::exhaustive_enums,
    clippy::todo,
    reason = "Deferred"
)]
#![cfg_attr(
    test,
    allow(
        unused_crate_dependencies,
        reason = "Some dev dependencies are only used in tests"
    )
)]

pub(crate) mod import;
pub(crate) mod runtime;
pub mod schema;

mod error;
pub use error::Error;

// Re-export vello
#[cfg(feature = "vello")]
pub use vello;

pub use runtime::{
    BlurEdgeMode, Composition, FilterEffect, FilterLayerResult, RenderSink, Renderer, model,
};
