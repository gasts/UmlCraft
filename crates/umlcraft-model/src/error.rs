use crate::id::Id;

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ModelError {
    #[error("no element or presentation {0}")]
    NotFound(Id),
    #[error("id {0} is already in use")]
    DuplicateId(Id),
    #[error("{child} ({child_type}) cannot be owned by {owner} ({owner_type})")]
    InvalidOwner { owner: Id, owner_type: &'static str, child: Id, child_type: &'static str },
    #[error("index {index} out of range")]
    IndexOutOfRange { index: usize },
    #[error("the model root cannot be removed, moved or created")]
    Root,
    #[error("{0} still owns elements")]
    HasChildren(Id),
    #[error("{target} is still referenced by {referrer}")]
    StillReferenced { target: Id, referrer: Id },
    #[error("{target} is still shown on diagram {diagram}")]
    StillShown { target: Id, diagram: Id },
    #[error("{from} refers to {to}, which does not exist or has the wrong type")]
    InvalidReference { from: Id, to: Id },
    #[error("the metaclass of {0} cannot change")]
    KindChanged(Id),
    #[error("moving {0} there would create an ownership cycle")]
    OwnershipCycle(Id),
    #[error("generalization {0} would create a cycle")]
    GeneralizationCycle(Id),
    #[error("{0} is not a diagram")]
    NotADiagram(Id),
    #[error("diagram {0} still has presentations")]
    DiagramNotEmpty(Id),
    #[error("edges are still attached to presentation {0}")]
    EdgesAttached(Id),
    #[error("presentation {0} cannot change between node and edge")]
    ShapeChanged(Id),
    #[error("invalid model: {0}")]
    Invalid(String),
}

pub type Result<T, E = ModelError> = std::result::Result<T, E>;
