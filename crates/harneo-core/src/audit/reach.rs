//! What an artifact can reach. Filled in by the reach ticket; until then every list is empty.

use super::{AuditInput, Reach};

/// The reach of the artifact behind `input`, with sorted lists.
pub(crate) fn infer(_input: &AuditInput) -> Reach {
    let mut reach = Reach::default();
    reach.normalize();
    reach
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ArtifactKind;

    #[test]
    fn reach_is_empty_until_inference_lands() {
        let input = AuditInput::with_files(ArtifactKind::Skill, "s", "s", &[("s/SKILL.md", "hi")]);
        assert!(infer(&input).is_empty());
    }
}
