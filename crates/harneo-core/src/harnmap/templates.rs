//! The eight categories and their templates: static data, one skeleton of stages per category.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::Error;

/// A profile's category; it picks the Harnmap template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum CategoryId {
    Software,
    Research,
    Writing,
    Video,
    Social,
    Data,
    Design,
    Custom,
}

impl CategoryId {
    /// Every category, in the order the picker shows them.
    pub const ALL: [CategoryId; 8] = [
        Self::Software,
        Self::Research,
        Self::Writing,
        Self::Video,
        Self::Social,
        Self::Data,
        Self::Design,
        Self::Custom,
    ];

    /// The id as stored in the database.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Software => "software",
            Self::Research => "research",
            Self::Writing => "writing",
            Self::Video => "video",
            Self::Social => "social",
            Self::Data => "data",
            Self::Design => "design",
            Self::Custom => "custom",
        }
    }
}

impl FromStr for CategoryId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Self::ALL
            .into_iter()
            .find(|c| c.as_str() == s)
            .ok_or_else(|| Error::InvalidProfile {
                reason: format!("unknown category {s:?}"),
            })
    }
}

/// How a map's flow is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Shape {
    Line,
    Cycle,
    Diamond,
    Parallel,
}

/// One stage of a template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TemplateStage {
    pub id: &'static str,
    pub name: &'static str,
}

/// A return path between two stages, by stage id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TemplateLoop {
    pub from: &'static str,
    pub to: &'static str,
    pub label: &'static str,
}

/// A named group of stages (video's pre-production, production and post-production).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TemplatePhase {
    pub name: &'static str,
    pub stages: Vec<&'static str>,
}

/// A category's template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Template {
    pub id: CategoryId,
    pub label: &'static str,
    /// `None` for Custom: Claude chooses the shape at the first draw.
    pub shape: Option<Shape>,
    pub stages: Vec<TemplateStage>,
    pub loops: Vec<TemplateLoop>,
    pub phases: Vec<TemplatePhase>,
    /// Three starters for the draw description.
    pub suggestions: [&'static str; 3],
}

/// Alias used by the app: a category is its template.
pub type Category = Template;

const fn st(id: &'static str, name: &'static str) -> TemplateStage {
    TemplateStage { id, name }
}

const fn lp(from: &'static str, to: &'static str, label: &'static str) -> TemplateLoop {
    TemplateLoop { from, to, label }
}

fn template(
    id: CategoryId,
    label: &'static str,
    shape: Option<Shape>,
    stages: Vec<TemplateStage>,
    loops: Vec<TemplateLoop>,
    suggestions: [&'static str; 3],
) -> Template {
    Template {
        id,
        label,
        shape,
        stages,
        loops,
        phases: Vec::new(),
        suggestions,
    }
}

/// The eight templates, in picker order.
pub fn templates() -> Vec<Template> {
    let line = Some(Shape::Line);
    vec![
        template(
            CategoryId::Software,
            "Software",
            line,
            vec![
                st("idea", "Idea"),
                st("spec", "Spec"),
                st("plan", "Plan"),
                st("build", "Build"),
                st("test", "Test"),
                st("review", "Review"),
                st("ship", "Ship"),
            ],
            vec![lp("review", "build", "changes requested")],
            [
                "Spec first, always.",
                "TDD lives inside Build.",
                "Review sends work back to Build.",
            ],
        ),
        template(
            CategoryId::Research,
            "Research",
            line,
            vec![
                st("question", "Question"),
                st("sources", "Sources"),
                st("read-extract", "Read & extract"),
                st("analyse", "Analyse"),
                st("synthesise", "Synthesise"),
                st("verify", "Verify"),
                st("report", "Report"),
            ],
            vec![
                lp("analyse", "read-extract", "needs more material"),
                lp("verify", "sources", "a claim lacks a source"),
            ],
            [
                "Every claim needs a source.",
                "Reading notes live inside Read & extract.",
                "Verify sends me back to Sources.",
            ],
        ),
        template(
            CategoryId::Writing,
            "Writing",
            line,
            vec![
                st("brief", "Brief"),
                st("research", "Research"),
                st("outline", "Outline"),
                st("draft", "Draft"),
                st("edit", "Edit"),
                st("fact-check", "Fact-check"),
                st("publish", "Publish"),
            ],
            vec![
                lp("edit", "draft", "rewrite"),
                lp("fact-check", "draft", "fix the facts"),
            ],
            [
                "Outline before any draft.",
                "Style rules apply to every Edit.",
                "Fact-check sends work back to Draft.",
            ],
        ),
        Template {
            phases: vec![
                TemplatePhase {
                    name: "Pre-production",
                    stages: vec!["concept", "script", "storyboard"],
                },
                TemplatePhase {
                    name: "Production",
                    stages: vec!["shoot-assets"],
                },
                TemplatePhase {
                    name: "Post-production",
                    stages: vec!["edit", "sound-captions", "publish"],
                },
            ],
            ..template(
                CategoryId::Video,
                "Video",
                Some(Shape::Parallel),
                vec![
                    st("concept", "Concept"),
                    st("script", "Script"),
                    st("storyboard", "Storyboard"),
                    st("shoot-assets", "Shoot & assets"),
                    st("edit", "Edit"),
                    st("sound-captions", "Sound & captions"),
                    st("publish", "Publish"),
                ],
                vec![],
                [
                    "Script before any shooting.",
                    "Captions run next to the edit.",
                    "Sound work happens beside Edit.",
                ],
            )
        },
        template(
            CategoryId::Social,
            "Social media",
            Some(Shape::Cycle),
            vec![
                st("plan", "Plan"),
                st("ideate", "Ideate"),
                st("create", "Create"),
                st("adapt", "Adapt per platform"),
                st("schedule", "Schedule"),
                st("engage", "Engage"),
                st("measure", "Measure"),
            ],
            vec![lp("measure", "plan", "what the numbers say")],
            [
                "One idea, adapted per platform.",
                "Replies belong to Engage.",
                "Measure feeds the next Plan.",
            ],
        ),
        template(
            CategoryId::Data,
            "Data",
            line,
            vec![
                st("question", "Question"),
                st("collect", "Collect"),
                st("clean", "Clean"),
                st("explore", "Explore"),
                st("model", "Model"),
                st("visualise", "Visualise"),
                st("report", "Report"),
            ],
            vec![
                lp("clean", "explore", "check the cleaned data"),
                lp("explore", "clean", "found dirty data"),
                lp("model", "explore", "rethink the features"),
            ],
            [
                "Never touch the raw data.",
                "Exploring and cleaning feed each other.",
                "A model that fails sends me back to Explore.",
            ],
        ),
        template(
            CategoryId::Design,
            "Design",
            Some(Shape::Diamond),
            vec![
                st("brief", "Brief"),
                st("research", "Research"),
                st("define", "Define"),
                st("concepts", "Concepts"),
                st("prototype", "Prototype"),
                st("critique", "Critique"),
                st("hand-off", "Hand-off"),
            ],
            vec![lp("critique", "prototype", "iterate")],
            [
                "Define the problem before any concept.",
                "Critique always follows a prototype.",
                "Hand-off carries the design tokens.",
            ],
        ),
        template(
            CategoryId::Custom,
            "Custom",
            None,
            vec![st("open", "Open")],
            vec![],
            [
                "Describe the steps your work goes through.",
                "Say which steps repeat or loop back.",
                "Name what must always be on.",
            ],
        ),
    ]
}
