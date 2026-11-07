//! Harnmap: a profile seen as a factory. A profile's category picks a [template](templates).

mod templates;
#[cfg(test)]
mod templates_tests;

pub use templates::{
    templates, Category, CategoryId, Shape, Template, TemplateLoop, TemplatePhase, TemplateStage,
};
