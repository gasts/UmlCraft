//! Maps Astah's object graph onto an UmlCraft model.
//!
//! Astah follows the UML 1.4 metamodel (`JP.co.esm.caddies.uml.*`) plus its own presentation
//! classes (`JP.co.esm.caddies.jomt.jmodel.*`). Only what UmlCraft can represent is imported:
//! packages, classifiers with their features, use cases, components, nodes, artifacts,
//! instances and links, the relationships between them, activities, interactions with their
//! interaction uses, and state machines; class, use case, sequence, activity, state machine,
//! component and deployment diagrams. Everything else is counted in [`Imported::skipped`].

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap, HashSet};

use umlcraft_model::{
    ActivityNodeKind, ActivityNodeProps, Aggregation, AssociationEnd, AssociationProps,
    AttributeProps, ClassifierProps, CombinedFragmentProps, CommentProps, DiagramKind,
    DirectedProps, EdgeLabels, ElementData, ElementKind, ExtendProps, GeneralizationProps, Id,
    IncludeProps, InstanceKind, InstanceProps, InteractionOperandProps, InteractionOperator,
    InteractionUseProps, InternalTransitionProps, LifelineProps, LineShape, LinkEnd, LinkProps,
    MessageGate, MessageProps, MessageSort, Model, Multiplicity, Navigability, Op, OperationProps,
    ParameterDirection, ParameterProps, Point, Presentation, PseudostateKind, Qualifier, Rect,
    Regions, Shape, SlotProps, StateProps, Style, TaggedValue, TemplateParameterKind,
    TemplateParameterProps, TransitionProps, TypeExpr, TypeRef, Visibility,
};
use umlcraft_notation::palette;
use umlcraft_notation::route;
use umlcraft_notation::scene::{self, Outline};

use crate::jser::{Graph, ObjId, Value};

/// The result of an import.
#[derive(Debug)]
pub struct Imported {
    pub model: Model,
    /// Astah classes of model elements and diagrams that were not imported, with how often
    /// they occurred. Presentations of skipped elements are left out silently.
    pub skipped: BTreeMap<String, usize>,
    /// Astah classes of elements UmlCraft knows but could not place in the model (no fitting
    /// owner, or the model refused them), with how often.
    pub failed: BTreeMap<String, usize>,
    /// Shapes on diagrams that were left out although their element was imported: the model
    /// refused them, or their ends did not resolve.
    pub lost_presentations: usize,
}

/// Astah's default fill color, not worth carrying over as an explicit style.
const DEFAULT_FILL: &str = "#FFFFCC";
/// How far right of its first stored point Astah draws the loop of a message to self.
const SELF_LOOP: f32 = 49.9;

/// Leftmost and topmost position of imported diagram content.
const MARGIN: f32 = 20.0;

/// Typed access to the fields of Astah objects.
#[derive(Clone, Copy)]
struct Reader<'g> {
    g: &'g Graph,
}

impl<'g> Reader<'g> {
    /// Unqualified class name, e.g. `UClassifierImp`.
    fn class(&self, id: ObjId) -> &'g str {
        let name = self.g.class_name(id).unwrap_or_default();
        name.rsplit('.').next().unwrap_or(name)
    }

    /// The first non-null object in a field of this name; a subclass may declare an unused
    /// field with the same name as the one that holds the value.
    fn obj(&self, id: ObjId, field: &str) -> Option<ObjId> {
        self.g.fields(id, field).find_map(Value::as_object)
    }

    /// The objects of a collection field.
    fn list(&self, id: ObjId, field: &str) -> Vec<ObjId> {
        self.obj(id, field)
            .map(|list| self.g.collection(list).into_iter().filter_map(Value::as_object).collect())
            .unwrap_or_default()
    }

    /// Text of a field: a plain string, or the body or label of the value object (names,
    /// expressions and enumeration kinds are such objects). Astah nests these one or two levels
    /// deep; a longer chain is a damaged file, possibly a cycle, and gives no text.
    fn text(&self, id: ObjId, field: &str) -> String {
        let mut current = self.obj(id, field);
        for _ in 0..16 {
            let Some(value) = current else { break };
            if let Some(s) = self.g.string(value) {
                return s.to_owned();
            }
            current = self.obj(value, "body").or_else(|| self.obj(value, "label"));
        }
        String::new()
    }

    fn name(&self, id: ObjId) -> String {
        self.text(id, "name")
    }

    /// A finite number; NaN and infinity would make the model unsavable.
    fn num(&self, id: ObjId, field: &str) -> Option<f64> {
        self.g.field(id, field).and_then(Value::as_f64).filter(|n| n.is_finite())
    }

    fn flag(&self, id: ObjId, field: &str) -> bool {
        self.g.bool_field(id, field).unwrap_or(false)
    }

    /// The numbers of an array or collection field.
    fn numbers(&self, id: ObjId, field: &str) -> Vec<f64> {
        self.obj(id, field)
            .map(|a| {
                let numbers = self.g.collection(a).into_iter().filter_map(Value::as_f64);
                numbers.filter(|n| n.is_finite()).collect()
            })
            .unwrap_or_default()
    }

    fn point(&self, id: ObjId) -> Option<Point> {
        Some(Point::new(self.num(id, "x")? as f32, self.num(id, "y")? as f32))
    }

    fn points(&self, id: ObjId, field: &str) -> Vec<Point> {
        self.list(id, field).into_iter().filter_map(|p| self.point(p)).collect()
    }

    fn stereotypes(&self, id: ObjId) -> Vec<String> {
        self.list(id, "stereotype").into_iter().map(|s| self.name(s)).collect()
    }

    fn tagged_values(&self, id: ObjId) -> Vec<(String, String)> {
        self.list(id, "taggedValue")
            .into_iter()
            .map(|t| (self.text(t, "tag"), self.text(t, "value")))
            .collect()
    }

    /// A string-to-string map field such as a presentation's `styleMap`.
    fn style(&self, id: ObjId, key: &str) -> Option<String> {
        let map = self.obj(id, "styleMap")?;
        let entries = self.g.collection(map);
        entries.chunks(2).find_map(|pair| {
            let [k, v] = pair else { return None };
            let k = self.g.string(k.as_object()?)?;
            (k == key).then(|| self.g.string(v.as_object()?).map(str::to_owned)).flatten()
        })
    }

    fn visibility(&self, id: ObjId) -> Visibility {
        let mut label = self.text(id, "visibility");
        if label.is_empty()
            && let Some(ownership) = self.obj(id, "namespaceOwnership")
        {
            label = self.text(ownership, "visibility");
        }
        match label.as_str() {
            "private" => Visibility::Private,
            "protected" => Visibility::Protected,
            "package" => Visibility::Package,
            _ => Visibility::Public,
        }
    }

    /// Astah stores `*` as -1 and "not set" as -100; textual bounds cannot be represented.
    fn multiplicity(&self, id: ObjId) -> Option<Multiplicity> {
        let range = *self.list(self.obj(id, "multiplicity")?, "multiplicityRange").first()?;
        if !self.text(range, "lowerValue").is_empty() || !self.text(range, "upperValue").is_empty()
        {
            return None;
        }
        let bound = |field| self.g.field(range, field).and_then(Value::as_i64);
        let (lower, upper) = (bound("lower")?, bound("upper")?);
        let upper = match upper {
            -1 => None,
            u if u >= 1 => Some(u32::try_from(u).ok()?),
            _ => return None,
        };
        // A lone "*" has no lower bound (-1); "0..*" has 0.
        let (lower, star) = match lower {
            -1 => (0, true),
            l if l >= 0 => (u32::try_from(l).ok()?, false),
            _ => return None,
        };
        let m = Multiplicity { lower, upper, star: star && upper.is_none() };
        m.to_string().parse::<Multiplicity>().ok()
    }
}

/// `actionType` of the action behind a message.
const CREATE: i64 = 0;
const REPLY: i64 = 2;
const DESTROY: i64 = 4;

fn action_type(r: Reader<'_>, message: ObjId) -> Option<i64> {
    r.g.field(r.obj(message, "action")?, "actionType").and_then(Value::as_i64)
}

/// Astah's prefixes of its language profiles' tagged values and UmlCraft's names for them.
const LANGUAGE_TAGS: [(&str, &str); 3] =
    [("jude.java.", "java."), ("jude.c_sharp.", "csharp."), ("jude.c_plus.", "cpp.")];

/// A language profile's tagged value under UmlCraft's name: `jude.java.transient` becomes
/// `java.transient`. Other keys stay as they are.
fn neutral_tag(key: &str) -> String {
    LANGUAGE_TAGS
        .iter()
        .find_map(|(astah, ours)| key.strip_prefix(astah).map(|rest| format!("{ours}{rest}")))
        .unwrap_or_else(|| key.to_owned())
}

/// Tagged values Astah uses for itself. Those of its language profiles stay: they hold
/// modifiers such as Java's `transient` (`jude.java.transient`), which UML has no place for.
fn is_internal_tag(key: &str) -> bool {
    let profile = LANGUAGE_TAGS.iter().any(|(astah, _)| key.starts_with(astah));
    (key.starts_with("jude.") && !profile)
        || matches!(key, "hiddenDependency" | "matrix_access_type")
}

/// `ty` followed by `modifier` (`[]`, `*` …); a bound template takes it on directly.
fn with_modifier(ty: TypeRef, modifier: String) -> TypeRef {
    if modifier.is_empty() {
        return ty;
    }
    match ty {
        TypeRef::Expr(mut e) if e.modifier.is_empty() => {
            e.modifier = modifier;
            TypeRef::Expr(e)
        }
        base => TypeRef::Expr(Box::new(TypeExpr {
            base,
            arguments: Vec::new(),
            prefix: String::new(),
            modifier,
        })),
    }
}

/// Stereotypes such as "Java Class" or "C# Method" mark the target language of code generation;
/// Astah does not display them.
fn is_language_marker(stereotype: &str) -> bool {
    stereotype.split_once(' ').is_some_and(|(language, what)| {
        matches!(language, "Java" | "C#" | "C++")
            && !what.is_empty()
            && !what.contains(' ')
            && what.starts_with(char::is_uppercase)
    })
}

/// Astah marks elements of its ER, data flow and requirement notations with tagged values on
/// otherwise plain UML elements.
fn is_foreign_notation(tags: &[(String, String)]) -> bool {
    foreign_notation(tags).is_some()
}

/// The notation that tagged values mark, as reported among the skipped kinds. Data flow
/// diagrams and flowcharts are activity diagrams with such tags.
fn foreign_notation(tags: &[(String, String)]) -> Option<&'static str> {
    tags.iter().find_map(|(key, value)| {
        if key.starts_with("jude.er.") || (key == "jude.type" && value.starts_with("er_")) {
            Some("EntityRelationship")
        } else if key.starts_with("jude.dfd.") {
            Some("DataFlowDiagram")
        } else if key.starts_with("jude.biz.") {
            Some("Flowchart")
        } else if key.starts_with("jude.require.") {
            Some("Requirement")
        } else {
            None
        }
    })
}

/// Astah's names for regions it creates, `Region0`, `Region1` …, or none.
fn is_default_region_name(name: &str) -> bool {
    name.is_empty() || name.trim_end_matches(|c: char| c.is_ascii_digit()) == "Region"
}

/// Names Astah gives control nodes, final states and pseudostates by itself, such as
/// `ActivityNode0`, `ForkNode1` or `InitialPseudostate0`, and the ones UmlCraft's export hands
/// out.
fn is_default_node_name(name: &str) -> bool {
    let stem = name.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end();
    matches!(
        stem,
        "" | "ActivityNode"
            | "InitialNode"
            | "ActivityFinal"
            | "FinalNode"
            | "FlowFinal"
            | "FlowFinalNode"
            | "ForkNode"
            | "JoinNode"
            | "DecisionNode"
            | "MergeNode"
            | "Decision Node & Merge Node"
            | "InitialPseudostate"
            | "FinalState"
            | "ChoicePseudostate"
            | "JunctionPseudostate"
            | "ForkPseudostate"
            | "JoinPseudostate"
            | "ShallowHistoryPseudostate"
            | "DeepHistoryPseudostate"
    )
}

/// The name Astah gives a dimension of lanes it creates by itself.
fn is_default_dimension_name(name: &str) -> bool {
    name.strip_prefix("Dimension").is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()))
}

struct Importer<'g> {
    r: Reader<'g>,
    model: Model,
    ids: HashMap<ObjId, Id>,
    skipped: BTreeMap<String, usize>,
    failed: BTreeMap<String, usize>,
    lost_presentations: usize,
    features: Vec<(Id, ObjId)>,
    /// Features already in `features`.
    queued: HashSet<ObjId>,
    /// Calls that have a reply message.
    replied: HashSet<ObjId>,
    relationships: Vec<(Id, ObjId)>,
    /// Instance specifications, created once their classifiers exist.
    instances: Vec<(Id, ObjId)>,
    /// Interaction uses and the Astah interaction they refer to.
    references: Vec<(Id, ObjId)>,
    /// Activity graphs and state machines with the element they belong to.
    behaviors: Vec<(Id, ObjId)>,
    diagrams: Vec<(Id, ObjId)>,
    /// Template classifiers, whose parameters are created once every classifier exists.
    templates: Vec<(Id, ObjId)>,
    /// Bound classes, whose bindings are set once every template parameter exists.
    bound: Vec<(Id, ObjId)>,
    /// Partitions visited, including the dimensions that are left out.
    dimensions: HashSet<ObjId>,
    /// Bound types `type_of` may still expand for the type it was asked for.
    type_budget: Cell<usize>,
}

/// Most bound types expanded for one type; real ones such as `Map<String, List<Order>>` need
/// a handful.
const MAX_TYPE_NODES: usize = 1000;

pub fn import(graph: &Graph) -> Option<Imported> {
    let r = Reader { g: graph };
    let root = project_model(&r)?;
    let model = Model::new(r.name(root));
    let mut im = Importer {
        r,
        ids: HashMap::from([(root, model.root())]),
        model,
        skipped: BTreeMap::new(),
        failed: BTreeMap::new(),
        lost_presentations: 0,
        features: Vec::new(),
        queued: HashSet::new(),
        replied: HashSet::new(),
        relationships: Vec::new(),
        instances: Vec::new(),
        references: Vec::new(),
        behaviors: Vec::new(),
        diagrams: Vec::new(),
        templates: Vec::new(),
        bound: Vec::new(),
        dimensions: HashSet::new(),
        type_budget: Cell::new(MAX_TYPE_NODES),
    };
    let root_id = im.model.root();
    im.walk(root, root_id);
    im.template_parameters();
    im.bindings();
    for (owner, feature) in std::mem::take(&mut im.features) {
        im.feature(owner, feature);
    }
    // Dependencies may connect instances, so they come first.
    for (owner, instance) in std::mem::take(&mut im.instances) {
        im.instance(owner, instance);
    }
    im.links();
    for (owner, relationship) in std::mem::take(&mut im.relationships) {
        im.relationship(owner, relationship);
    }
    for (owner, behavior) in std::mem::take(&mut im.behaviors) {
        if im.r.class(behavior) == "UStateMachineImp" {
            im.state_machine(owner, behavior);
        } else {
            im.activity(owner, behavior);
        }
    }
    im.unowned_diagrams();
    for (owner, diagram) in std::mem::take(&mut im.diagrams) {
        im.diagram(owner, diagram);
    }
    im.resolve_references();
    Some(Imported {
        model: im.model,
        skipped: im.skipped,
        failed: im.failed,
        lost_presentations: im.lost_presentations,
    })
}

/// The project's model: `EntityRoot.entry["ProjectModel"]`, or else the first model object.
fn project_model(r: &Reader<'_>) -> Option<ObjId> {
    let g = r.g;
    let all = || (0..g.objects.len()).map(ObjId);
    let from_entry = all().filter(|&id| r.class(id) == "EntityRoot").find_map(|root| {
        let entries = g.collection(r.obj(root, "entry")?);
        entries.chunks(2).find_map(|pair| {
            let [k, v] = pair else { return None };
            (g.string(k.as_object()?)? == "ProjectModel").then(|| v.as_object()).flatten()
        })
    });
    from_entry.or_else(|| all().find(|&id| r.class(id) == "UModelImp"))
}

impl Importer<'_> {
    fn skip(&mut self, object: ObjId) {
        *self.skipped.entry(self.r.class(object).to_owned()).or_default() += 1;
    }

    /// Name, visibility, stereotypes, tagged values and documentation of an Astah element.
    fn data(&self, object: ObjId, kind: ElementKind, hide_stereotypes: &[&str]) -> ElementData {
        let mut data = ElementData::new(self.r.name(object), kind);
        data.visibility = self.r.visibility(object);
        data.stereotypes = self
            .r
            .stereotypes(object)
            .into_iter()
            .filter(|s| {
                !s.is_empty() && !hide_stereotypes.contains(&s.as_str()) && !is_language_marker(s)
            })
            .collect();
        data.tagged_values = self
            .r
            .tagged_values(object)
            .into_iter()
            .filter(|(key, _)| !key.is_empty() && !is_internal_tag(key))
            .map(|(key, value)| TaggedValue { key: neutral_tag(&key), value })
            .collect();
        data.documentation = self.r.text(object, "definition");
        data
    }

    /// Inserts an element under `owner`, or under the nearest ancestor that may own it.
    ///
    /// Each Astah object becomes at most one element. An object reached a second time means a
    /// damaged file in which, say, a package owns itself; refusing it ends the walks that
    /// recurse into what they create.
    fn create(&mut self, owner: Id, data: ElementData, object: Option<ObjId>) -> Option<Id> {
        if object.is_some_and(|o| self.ids.contains_key(&o)) {
            return None;
        }
        let owner = std::iter::once(owner)
            .chain(self.model.ancestors(owner))
            .find(|&o| self.model.get(o).is_some_and(|e| e.kind().can_own(&data.kind)));
        let id = Id::generate();
        let inserted = owner.map(|owner| {
            let index = self.model.children(owner).len();
            self.model.apply(Op::InsertElement { id, owner, index, data })
        });
        match inserted {
            Some(Ok(_)) => {
                if let Some(object) = object {
                    self.ids.insert(object, id);
                }
                Some(id)
            }
            _ => {
                if let Some(object) = object {
                    *self.failed.entry(self.r.class(object).to_owned()).or_default() += 1;
                }
                None
            }
        }
    }

    /// Whether a classifier of Astah's stands for a template parameter. Some classes declare
    /// `templateParameter` as a (usually empty) list.
    fn is_template_parameter(&self, classifier: ObjId) -> bool {
        ["owningParameter", "templateParameter"].iter().any(|&field| {
            self.r.obj(classifier, field).is_some_and(|p| self.r.class(p) != "ArrayList")
        })
    }

    fn kind_of(&self, object: ObjId) -> Option<&ElementKind> {
        self.ids.get(&object).and_then(|id| self.model.get(*id)).map(|e| e.kind())
    }

    fn classifier(&self, object: ObjId) -> Option<Id> {
        self.kind_of(object).filter(|k| k.is_classifier())?;
        self.ids.get(&object).copied()
    }

    /// Creates the elements owned by `object`. Features, relationships and diagrams wait until
    /// every classifier exists, so their references can be resolved.
    fn walk(&mut self, object: ObjId, id: Id) {
        // Behaviors are not owned elements; the element they describe lists them.
        for behavior in self.r.list(object, "behavior") {
            match self.r.class(behavior) {
                "UActivityGraphImp" | "UStateMachineImp" => self.behaviors.push((id, behavior)),
                _ => self.skip(behavior),
            }
        }
        for ownership in self.r.list(object, "ownedElementOwnership") {
            let Some(child) = self.r.obj(ownership, "ownedElement") else { continue };
            if is_foreign_notation(&self.r.tagged_values(child)) {
                self.skip(child);
                continue;
            }
            match self.r.class(child) {
                "UPackageImp" | "USubsystemImp" | "UModelImp" => {
                    let mut data = self.data(child, ElementKind::Package, &[]);
                    // Astah keeps subsystems as a class of their own; UmlCraft as a stereotype.
                    if self.r.class(child) == "USubsystemImp"
                        && !data.stereotypes.iter().any(|s| s == palette::SUBSYSTEM)
                    {
                        data.stereotypes.insert(0, palette::SUBSYSTEM.to_owned());
                    }
                    if let Some(package) = self.create(id, data, Some(child)) {
                        self.walk(child, package);
                    }
                }
                "UClassifierImp" | "UInterfaceImp" | "UActorImp" | "UUseCaseImp"
                | "UComponentImp" | "UArtifactImp" | "UNodeImp" => {
                    // Anonymous classifiers bind a template to arguments; they live on as type
                    // expressions such as `List<Projekt>` (see `type_of`), and as bound classes
                    // where a diagram shows them, a lifeline represents them (`baseInv`) or a
                    // relationship ends at them (`implements Comparable<Order>`, an association
                    // to `List<User>`).
                    let bound = !self.r.list(child, "templateBinding").is_empty();
                    let referenced = [
                        "presentation",
                        "baseInv",
                        "supplierDependency",
                        "clientDependency",
                        "specialization",
                        "generalization",
                        "associationEnd",
                    ]
                    .iter()
                    .any(|field| !self.r.list(child, field).is_empty());
                    if bound && self.r.name(child).is_empty() && !referenced {
                        continue;
                    }
                    // Template parameters are classifiers owned by a template parameter; they
                    // are created with their template (see `template_parameters`).
                    if self.is_template_parameter(child) {
                        continue;
                    }
                    let (kind, hide) = self.classifier_kind(child);
                    let data = self.data(child, kind, hide);
                    if let Some(classifier) = self.create(id, data, Some(child)) {
                        if self.r.obj(child, "ownedTemplateSignatrue").is_some() {
                            self.templates.push((classifier, child));
                        }
                        if bound {
                            self.bound.push((classifier, child));
                        }
                        // The feature lists hold the order Astah displays; ownership does not.
                        for list in ["structuralFeature", "behavioralFeature"] {
                            for feature in self.r.list(child, list) {
                                if self.queued.insert(feature) {
                                    self.features.push((classifier, feature));
                                }
                            }
                        }
                        self.walk(child, classifier);
                    }
                }
                "UExtensionPointImp" => {
                    let data = self.data(child, ElementKind::ExtensionPoint, &[]);
                    self.create(id, data, Some(child));
                }
                // Features listed by their classifier are queued above already.
                "UAttributeImp" | "UOperationImp" => {
                    if self.queued.insert(child) {
                        self.features.push((id, child));
                    }
                }
                // Parameters come with their operation, association ends with their association.
                "JUParameterImp" | "UParameterImp" | "UAssociationEndImp" => {}
                "UAssociationImp"
                | "UGeneralizationImp"
                | "UDependencyImp"
                | "UUsageImp"
                | "UAbstractionImp"
                | "URealizationImp"
                | "UIncludeImp"
                | "UExtendImp"
                | "UTemplateBindingImp" => {
                    self.relationships.push((id, child));
                }
                "UDiagramImp" | "USequenceDiagramImp" => self.diagrams.push((id, child)),
                "UComponentInstanceImp" | "UNodeInstanceImp" | "UObjectImp" => {
                    self.instances.push((id, child));
                }
                // Constraints on association ends come with their association.
                "UConstraintImp"
                    if self
                        .r
                        .list(child, "constrainedElement")
                        .iter()
                        .all(|&e| self.r.class(e) == "UAssociationEndImp") => {}
                _ => self.skip(child),
            }
        }
    }

    /// Diagrams outside the ownership tree. A sequence diagram that belongs to a classifier, use
    /// case or operation is reachable only through its collaboration, which names that element
    /// as `representedClassifier` or `representedOperation`; an operation's interaction goes to
    /// its class (see `create`). Others go to the model root.
    fn unowned_diagrams(&mut self) {
        let queued: HashSet<ObjId> = self.diagrams.iter().map(|&(_, d)| d).collect();
        let r = self.r;
        for diagram in (0..r.g.objects.len()).map(ObjId) {
            if !matches!(r.class(diagram), "UDiagramImp" | "USequenceDiagramImp")
                || queued.contains(&diagram)
            {
                continue;
            }
            let collaboration = r.obj(diagram, "collaboration");
            let owner = ["representedClassifier", "representedOperation"]
                .iter()
                .find_map(|field| collaboration.and_then(|c| r.obj(c, field)))
                .and_then(|c| self.ids.get(&c).copied())
                .unwrap_or_else(|| self.model.root());
            self.diagrams.push((owner, diagram));
        }
    }

    fn classifier_kind(&self, object: ObjId) -> (ElementKind, &'static [&'static str]) {
        let props = ClassifierProps {
            is_abstract: self.r.flag(object, "abst"),
            is_leaf: self.r.flag(object, "leaf"),
            is_active: self.r.flag(object, "active"),
            binding: None,
        };
        let stereotypes = self.r.stereotypes(object);
        let has = |s: &str| stereotypes.iter().any(|t| t == s);
        let enum_first = stereotypes.first().is_some_and(|s| s == "enum");
        let constants = self
            .r
            .list(object, "structuralFeature")
            .iter()
            .any(|&a| self.r.stereotypes(a).iter().any(|s| s == "enum constant"));
        match self.r.class(object) {
            "UUseCaseImp" => (ElementKind::UseCase(props), &[]),
            "UComponentImp" => (ElementKind::Component(props), &[]),
            "UArtifactImp" => (ElementKind::Artifact(props), &[]),
            "UNodeImp" => (ElementKind::Node(props), &[]),
            "UInterfaceImp" => (ElementKind::Interface(props), &["interface"]),
            "UActorImp" => (ElementKind::Actor(props), &["actor"]),
            _ if has("interface") => (ElementKind::Interface(props), &["interface"]),
            _ if has("actor") => (ElementKind::Actor(props), &["actor"]),
            _ if has("enumeration") => (ElementKind::Enumeration(props), &["enumeration"]),
            // A Java enum imported by Astah: an enumeration when «enum» comes first and there
            // are «enum constant»s. Otherwise a class, which shows «enum» only in front.
            _ if enum_first && constants => (ElementKind::Enumeration(props), &["enum"]),
            _ if has("dataType") || has("datatype") => {
                (ElementKind::DataType(props), &["dataType", "datatype"])
            }
            _ if enum_first => (ElementKind::Class(props), &[]),
            _ => (ElementKind::Class(props), &["enum"]),
        }
    }

    /// Points interaction uses to the interactions they refer to, now that all exist.
    fn resolve_references(&mut self) {
        for (id, target) in std::mem::take(&mut self.references) {
            let Some(&interaction) = self.ids.get(&target) else { continue };
            if !matches!(
                self.model.get(interaction).map(|e| e.kind()),
                Some(ElementKind::Interaction)
            ) {
                continue;
            }
            let Some(e) = self.model.get(id) else { continue };
            let mut data = e.data().clone();
            if let ElementKind::InteractionUse(p) = &mut data.kind {
                p.refers_to = Some(interaction);
                let _ = self.model.apply(Op::UpdateElement { id, data });
            }
        }
    }

    /// An object, component instance or node instance of the first of its classifiers.
    fn instance(&mut self, owner: Id, object: ObjId) {
        let instance = match self.r.class(object) {
            "UComponentInstanceImp" => InstanceKind::Component,
            "UNodeInstanceImp" => InstanceKind::Node,
            _ => InstanceKind::Object,
        };
        let classifier =
            self.r.list(object, "classifiers").first().and_then(|&c| self.classifier(c));
        let data =
            self.data(object, ElementKind::Instance(InstanceProps { instance, classifier }), &[]);
        let Some(id) = self.create(owner, data, Some(object)) else { return };
        // Slots: the value is an instance whose name is the value's text.
        for slot in self.r.list(object, "slots") {
            let attribute = self.r.obj(slot, "attribute").and_then(|a| self.ids.get(&a).copied());
            let value = self.r.obj(slot, "value").map(|v| self.r.name(v)).unwrap_or_default();
            if attribute.is_none() || value.is_empty() {
                continue;
            }
            let data = self.data(slot, ElementKind::Slot(SlotProps { attribute, value }), &[]);
            self.create(id, data, Some(slot));
        }
    }

    /// Links between objects. Astah keeps them outside the ownership tree, reachable from their
    /// objects only; they go next to their first object. Navigability and aggregation of an end
    /// are tagged values.
    fn links(&mut self) {
        let r = self.r;
        for link in (0..r.g.objects.len()).map(ObjId).filter(|&o| r.class(o) == "ULinkImp") {
            let ends: Vec<LinkEnd> = r
                .list(link, "connections")
                .into_iter()
                .filter_map(|end| {
                    let instance = *self.ids.get(&r.obj(end, "instance")?)?;
                    let navigability = match self.tag(end, "object_navigable_type").as_str() {
                        "navigable" => Navigability::Navigable,
                        "non navigable" => Navigability::NonNavigable,
                        _ => Navigability::Unspecified,
                    };
                    let aggregation = match self.tag(end, "object_aggregation_kind").as_str() {
                        "aggregate" => Aggregation::Shared,
                        "composite" => Aggregation::Composite,
                        _ => Aggregation::None,
                    };
                    Some(LinkEnd { instance, role: r.name(end), navigability, aggregation })
                })
                .collect();
            let Ok(ends) = <[LinkEnd; 2]>::try_from(ends) else {
                self.skip(link);
                continue;
            };
            let association = r.obj(link, "association").and_then(|a| self.ids.get(&a).copied());
            let owner = self.model.get(ends[0].instance).and_then(|e| e.owner());
            let owner = owner.unwrap_or_else(|| self.model.root());
            let data = self.data(link, ElementKind::Link(LinkProps { ends, association }), &[]);
            self.create(owner, data, Some(link));
        }
    }

    /// The type of an attribute, parameter, qualifier or template parameter default. Astah marks
    /// arrays with a `jude.type_modifier` tag (`[][]`), or with one unset multiplicity range per
    /// dimension when the type was written as `Foo[]`. Like typed text (`member::parse_type`), a
    /// type that links to nothing in the model stays text.
    fn type_ref(&self, typed: ObjId) -> Option<TypeRef> {
        let ty = self.type_of(self.r.obj(typed, "type")?, 0)?;
        let mut modifier = self.tag(typed, "jude.type_modifier");
        if modifier.is_empty() {
            let unset = |range: &ObjId| {
                let bound = |field| self.r.g.field(*range, field).and_then(Value::as_i64);
                bound("lower") == Some(-100) && bound("upper") == Some(-100)
            };
            let ranges =
                self.r.obj(typed, "multiplicity").map(|m| self.r.list(m, "multiplicityRange"));
            modifier = "[]".repeat(ranges.unwrap_or_default().iter().filter(|r| unset(r)).count());
        }
        let ty = with_modifier(ty, modifier);
        Some(match ty {
            TypeRef::Expr(_) if ty.ids().is_empty() => {
                TypeRef::Text(umlcraft_notation::member::type_text(&self.model, &ty))
            }
            ty => ty,
        })
    }

    /// A type object: a classifier or template parameter of the model, a bound template such
    /// as `List<Projekt>` (an anonymous classifier whose template binding names the template
    /// and the arguments), or else the type's name as text.
    fn type_of(&self, ty: ObjId, depth: usize) -> Option<TypeRef> {
        if depth == 0 {
            self.type_budget.set(MAX_TYPE_NODES);
        }
        let r = self.r;
        let name = r.name(ty);
        let anonymous = name.is_empty() && !r.list(ty, "templateBinding").is_empty();
        // Types stay expressions even where a diagram shows the bound class.
        if let Some(&id) = self.ids.get(&ty).filter(|_| !anonymous)
            && self.model.get(id).is_some_and(|e| {
                e.kind().is_classifier() || matches!(e.kind(), ElementKind::TemplateParameter(_))
            })
        {
            return Some(TypeRef::Element(id));
        }
        if anonymous {
            return self.bound_type(ty, depth);
        }
        (!name.is_empty()).then_some(TypeRef::Text(name))
    }

    /// The template and arguments a classifier binds, as `Pair<int,Holder>`.
    fn bound_type(&self, ty: ObjId, depth: usize) -> Option<TypeRef> {
        if depth == 0 {
            self.type_budget.set(MAX_TYPE_NODES);
        }
        // The depth limit alone does not bound the work: arguments naming bound classes again
        // multiply at every level.
        let budget = self.type_budget.get().checked_sub(1)?;
        self.type_budget.set(budget);
        let r = self.r;
        let binding = r.list(ty, "templateBinding").first().copied();
        if let Some(binding) = binding.filter(|_| depth < 8)
            && let Some(signature) = r.obj(binding, "signature")
        {
            let base = r.obj(signature, "template").and_then(|t| self.type_of(t, depth + 1))?;
            let formals = r.list(signature, "ownedParameter");
            let mut substitutions = r.list(binding, "parameterSubstition");
            let position = |s: &ObjId| {
                let formal = r.obj(*s, "formal");
                formals.iter().position(|f| Some(*f) == formal).unwrap_or(usize::MAX)
            };
            substitutions.sort_by_key(position);
            let arguments =
                substitutions.iter().filter_map(|&s| self.actual(s, depth + 1)).collect();
            return Some(TypeRef::Expr(Box::new(TypeExpr {
                base,
                arguments,
                prefix: String::new(),
                modifier: String::new(),
            })));
        }
        None
    }

    /// Sets the bindings of bound classes, now that templates and their parameters exist.
    fn bindings(&mut self) {
        for (id, object) in std::mem::take(&mut self.bound) {
            let binding = self.bound_type(object, 0);
            let Some(e) = self.model.get(id) else { continue };
            let mut data = e.data().clone();
            if let ElementKind::Class(p) = &mut data.kind {
                p.binding = binding;
                let _ = self.model.apply(Op::UpdateElement { id, data });
            }
        }
    }

    /// The argument of a template parameter substitution: a type, or a value such as `20`.
    fn actual(&self, substitution: ObjId, depth: usize) -> Option<TypeRef> {
        let r = self.r;
        let argument = match r.list(substitution, "actual").first() {
            Some(&a) if r.class(a) == "UExpression" => Some(TypeRef::Text(r.text(a, "body"))),
            Some(&a) => self.type_of(a, depth),
            None => None,
        };
        let argument = argument.filter(|a| *a != TypeRef::Text(String::new())).or_else(|| {
            Some(self.tag(substitution, "actual_value"))
                .filter(|v| !v.is_empty())
                .map(TypeRef::Text)
        })?;
        Some(with_modifier(argument, self.tag(substitution, "jude.type_modifier")))
    }

    /// The value of one of Astah's tagged values, or an empty string.
    fn tag(&self, object: ObjId, key: &str) -> String {
        let tags = self.r.tagged_values(object);
        tags.into_iter().find(|(k, _)| k == key).map(|(_, v)| v).unwrap_or_default()
    }

    /// Creates the template parameters of template classifiers, before the features that use
    /// them. Astah's parameter owns a classifier that stands for it: its generalizations are
    /// the bounds, and a `jude.type` tag makes it a value parameter of that type.
    fn template_parameters(&mut self) {
        let r = self.r;
        for (owner, template) in std::mem::take(&mut self.templates) {
            let Some(signature) = r.obj(template, "ownedTemplateSignatrue") else { continue };
            for parameter in r.list(signature, "ownedParameter") {
                let Some(element) = r.obj(parameter, "ownedParameteredElement") else {
                    self.skip(parameter);
                    continue;
                };
                let value_type = Some(self.tag(element, "jude.type")).filter(|t| !t.is_empty());
                let bounds = r
                    .list(element, "generalization")
                    .into_iter()
                    .filter_map(|g| r.obj(g, "supertype"))
                    .filter_map(|s| self.type_of(s, 0))
                    .collect();
                // A value default is an expression whose body is the text.
                let default = r.obj(parameter, "defaultValue").and_then(|d| match r.g.string(d) {
                    Some(text) => Some(TypeRef::Text(text.to_owned())),
                    None if r.class(d) == "UExpression" => {
                        Some(r.text(d, "body")).filter(|b| !b.is_empty()).map(TypeRef::Text)
                    }
                    None => self.type_of(d, 0),
                });
                let props = TemplateParameterProps {
                    parameter: if value_type.is_some() {
                        TemplateParameterKind::Value
                    } else {
                        TemplateParameterKind::Type
                    },
                    type_ref: value_type.map(TypeRef::Text),
                    bounds,
                    default,
                    is_variadic: false,
                };
                let data = self.data(element, ElementKind::TemplateParameter(props), &[]);
                self.create(owner, data, Some(element));
            }
        }
    }

    fn feature(&mut self, owner: Id, object: ObjId) {
        let is_static = self.r.text(object, "ownerScope") == "classifier";
        if self.r.class(object) == "UAttributeImp" {
            let owner_is_enum = matches!(
                self.model.get(owner).map(|e| e.kind()),
                Some(ElementKind::Enumeration(_))
            );
            // In a Java enum only the «enum constant»s are literals.
            let java_enum = self
                .r
                .obj(object, "owner")
                .is_some_and(|o| self.r.stereotypes(o).first().is_some_and(|s| s == "enum"));
            let constant = self.r.stereotypes(object).iter().any(|s| s == "enum constant");
            let kind = if owner_is_enum && (constant || !java_enum) {
                ElementKind::EnumerationLiteral
            } else {
                ElementKind::Attribute(AttributeProps {
                    type_ref: self.type_ref(object),
                    multiplicity: self.r.multiplicity(object),
                    default_value: self.r.text(object, "initialValue"),
                    is_static,
                    is_derived: false,
                    is_read_only: matches!(
                        self.r.text(object, "changeable").as_str(),
                        "frozen" | "readOnly"
                    ),
                })
            };
            let literal = matches!(kind, ElementKind::EnumerationLiteral);
            let data = self.data(object, kind, if literal { &["enum constant"] } else { &[] });
            self.create(owner, data, Some(object));
            return;
        }

        let parameters = self.r.list(object, "parameter");
        let r = self.r;
        let is_return = move |p: &ObjId| r.text(*p, "kind") == "return";
        // Astah shows a return type, "void" included, exactly when there is a return parameter.
        let return_type = parameters.iter().find(|p| is_return(p)).and_then(|&p| self.type_ref(p));
        let kind = ElementKind::Operation(OperationProps {
            return_type,
            is_static,
            is_abstract: self.r.flag(object, "isAbst"),
            is_leaf: self.r.flag(object, "leaf"),
            is_query: self.r.flag(object, "query"),
        });
        let data = self.data(object, kind, &[]);
        let Some(operation) = self.create(owner, data, Some(object)) else { return };
        for parameter in parameters.into_iter().filter(|p| !is_return(p)) {
            let direction = match self.r.text(parameter, "kind").as_str() {
                "out" => ParameterDirection::Out,
                "inout" => ParameterDirection::InOut,
                _ => ParameterDirection::In,
            };
            let kind = ElementKind::Parameter(ParameterProps {
                type_ref: self.type_ref(parameter),
                direction,
                multiplicity: self.r.multiplicity(parameter),
                default_value: self.r.text(parameter, "defaultValue"),
            });
            let data = self.data(parameter, kind, &[]);
            self.create(operation, data, Some(parameter));
        }
    }

    fn relationship(&mut self, owner: Id, object: ObjId) {
        let r = self.r;
        let mapped = |field: &str| r.obj(object, field).and_then(|o| self.ids.get(&o).copied());
        let first =
            |field: &str| r.list(object, field).first().and_then(|o| self.ids.get(o).copied());
        let class = r.class(object);
        let stereotypes = r.stereotypes(object);
        let kind = match class {
            "UAssociationImp" => {
                let ends: Vec<AssociationEnd> = r
                    .list(object, "connection")
                    .into_iter()
                    .filter_map(|end| self.association_end(end))
                    .collect();
                ends.try_into().ok().map(|ends| ElementKind::Association(AssociationProps { ends }))
            }
            // A template parameter's generalizations are its bounds (see `template_parameters`).
            "UGeneralizationImp"
                if r.obj(object, "subtype").is_some_and(|s| self.is_template_parameter(s)) =>
            {
                return;
            }
            "UGeneralizationImp" => {
                let specific = r.obj(object, "subtype").and_then(|o| self.classifier(o));
                let general = r.obj(object, "supertype").and_then(|o| self.classifier(o));
                specific.zip(general).map(|(specific, general)| {
                    ElementKind::Generalization(GeneralizationProps { specific, general })
                })
            }
            // A binding shown on a diagram is a «bind» dependency of the bound class on its
            // template; the class holds what it binds (see `bindings`). Bindings of types that
            // are not shown live on in the types.
            "UTemplateBindingImp" => {
                let template = r.obj(object, "signature").and_then(|s| r.obj(s, "template"));
                let client = r.obj(object, "boundElement").and_then(|o| self.classifier(o));
                let Some((client, supplier)) =
                    client.zip(template.and_then(|t| self.classifier(t)))
                else {
                    return;
                };
                let mut data = self.data(
                    object,
                    ElementKind::Dependency(DirectedProps { client, supplier }),
                    &["bind"],
                );
                data.stereotypes.insert(0, "bind".into());
                self.create(owner, data, Some(object));
                return;
            }
            // Astah's fields are swapped against UML: `addition` is the including use case.
            "UIncludeImp" => mapped("addition").zip(mapped("base")).map(|(including, addition)| {
                ElementKind::Include(IncludeProps { including, addition })
            }),
            "UExtendImp" => mapped("extension").zip(mapped("base")).map(|(extension, extended)| {
                ElementKind::Extend(ExtendProps {
                    extension,
                    extended,
                    extension_points: r
                        .list(object, "location")
                        .iter()
                        .filter_map(|p| self.ids.get(p).copied())
                        .collect(),
                    condition: r.text(object, "condition"),
                })
            }),
            _ => {
                let hidden = r
                    .tagged_values(object)
                    .iter()
                    .any(|(k, v)| k == "hiddenDependency" && v == "true");
                // Astah's own bookkeeping, not a dependency the user drew: nothing is lost.
                if hidden {
                    return;
                }
                let realize = class == "URealizationImp"
                    || stereotypes.iter().any(|s| s == "realize" || s == "realization");
                first("client").zip(first("supplier")).map(|(client, supplier)| {
                    let props = DirectedProps { client, supplier };
                    if realize {
                        ElementKind::Realization(props)
                    } else {
                        ElementKind::Dependency(props)
                    }
                })
            }
        };
        let Some(kind) = kind else {
            self.skip(object);
            return;
        };
        let hide: &[&str] = match &kind {
            ElementKind::Include(_) => &["include"],
            ElementKind::Extend(_) => &["extend"],
            ElementKind::Realization(_) => &["realize", "realization"],
            _ => &[],
        };
        let usage = matches!(kind, ElementKind::Dependency(_));
        let mut data = self.data(object, kind, hide);
        // A usage is a dependency marked «use», unless Astah made it a realization (a component
        // providing an interface on a deployment diagram).
        if class == "UUsageImp" && usage && !data.stereotypes.iter().any(|s| s == "use") {
            data.stereotypes.insert(0, "use".into());
        }
        self.create(owner, data, Some(object));
    }

    fn association_end(&self, end: ObjId) -> Option<AssociationEnd> {
        let r = self.r;
        let mut result = AssociationEnd::new(self.classifier(r.obj(end, "type")?)?);
        result.role = r.name(end);
        result.multiplicity = r.multiplicity(end);
        result.visibility = r.visibility(end);
        result.constraints = r
            .list(end, "constraint")
            .into_iter()
            .map(|c| Some(r.text(c, "body")).filter(|b| !b.is_empty()).unwrap_or_else(|| r.name(c)))
            .filter(|c| !c.is_empty())
            .collect();
        result.navigability = match r.text(end, "navigableType").as_str() {
            "navigable" => Navigability::Navigable,
            "non navigable" | "unnavigable" | "not navigable" => Navigability::NonNavigable,
            _ => Navigability::Unspecified,
        };
        result.aggregation = match r.text(end, "aggregation").as_str() {
            "composite" => Aggregation::Composite,
            "aggregate" | "shared" => Aggregation::Shared,
            _ => Aggregation::None,
        };
        result.is_read_only = matches!(r.text(end, "changeable").as_str(), "frozen" | "readOnly");
        result.is_static = r.text(end, "targetScope") == "classifier";
        result.default_value = r.text(end, "initialValue");
        // A qualifier belongs to the end whose participant its box is drawn at.
        result.qualifiers = r
            .list(end, "qualifier")
            .into_iter()
            .map(|q| Qualifier { name: r.name(q), type_ref: self.type_ref(q) })
            .collect();
        Some(result)
    }

    /// Creates an activity with its nodes, flows and partitions, and queues its diagram.
    /// Astah keeps UML 1.4's activity graphs: states of a state machine connected by
    /// transitions.
    fn activity(&mut self, owner: Id, graph: ObjId) {
        let r = self.r;
        let diagram = r.obj(graph, "diagram");
        let mut tags = r.tagged_values(graph);
        tags.extend(diagram.map(|d| r.tagged_values(d)).unwrap_or_default());
        if let Some(notation) = foreign_notation(&tags) {
            *self.skipped.entry(notation.to_owned()).or_default() += 1;
            return;
        }
        let data = self.data(graph, ElementKind::Activity, &[]);
        let Some(activity) = self.create(owner, data, Some(graph)) else { return };
        for partition in r.list(graph, "partition") {
            self.partition(activity, partition);
        }
        let vertices = r.obj(graph, "top").map(|top| r.list(top, "subvertex")).unwrap_or_default();
        for vertex in vertices {
            let Some(node) = self.activity_node_kind(vertex) else {
                self.skip(vertex);
                continue;
            };
            // Object nodes have a classifier in a state as their type: the classifier and the
            // states it is in. Astah keeps `filled, paid` typed in as one state of that name.
            let in_state = r.obj(vertex, "type").filter(|_| node == ActivityNodeKind::Object);
            let type_ref = in_state.and_then(|t| self.type_ref(t));
            let in_state = in_state
                .map(|t| r.list(t, "inStates").into_iter().map(|s| r.name(s)).collect::<Vec<_>>())
                .unwrap_or_default()
                .join(", ");
            let mut data = self.data(
                vertex,
                ElementKind::ActivityNode(ActivityNodeProps { node, type_ref, in_state }),
                &[],
            );
            // Astah names control nodes by itself; UmlCraft neither shows nor needs those names.
            if !matches!(node, ActivityNodeKind::Action | ActivityNodeKind::Object)
                && is_default_node_name(&data.name)
            {
                data.name.clear();
            }
            self.create(activity, data, Some(vertex));
        }
        for transition in r.list(graph, "transition") {
            let end = |field| r.obj(transition, field).and_then(|v| self.ids.get(&v).copied());
            let (Some(source), Some(target)) = (end("source"), end("target")) else {
                self.skip(transition);
                continue;
            };
            let guard = r.obj(transition, "guard").map(|g| r.name(g)).unwrap_or_default();
            let mut data = self.data(transition, ElementKind::flow(source, target), &[]);
            // Astah names a transition after its whole label, "trigger [guard] / effect".
            data.name = data.name.replace(&format!("[{guard}]"), "").trim().to_owned();
            if let ElementKind::ActivityEdge(p) = &mut data.kind {
                p.guard = guard;
            }
            self.create(activity, data, Some(transition));
        }
        if let Some(diagram) = diagram {
            self.diagrams.push((activity, diagram));
        }
    }

    /// Creates a state machine with its states and transitions, and queues its diagram. All
    /// of Astah's states are composite states; one with substates becomes a composite state in
    /// UmlCraft as well.
    fn state_machine(&mut self, owner: Id, machine: ObjId) {
        let r = self.r;
        let data = self.data(machine, ElementKind::StateMachine, &[]);
        let Some(id) = self.create(owner, data, Some(machine)) else { return };
        if let Some(top) = r.obj(machine, "top") {
            self.vertices(id, top);
        }
        for transition in r.list(machine, "transition") {
            let end = |field| r.obj(transition, field).and_then(|v| self.ids.get(&v).copied());
            let (Some(source), Some(target)) = (end("source"), end("target")) else {
                self.skip(transition);
                continue;
            };
            let named = |field| r.obj(transition, field).map(|o| r.name(o)).unwrap_or_default();
            let props = TransitionProps {
                source,
                target,
                trigger: named("trigger"),
                guard: named("guard"),
                effect: named("effect"),
            };
            let mut data = self.data(transition, ElementKind::Transition(props.clone()), &[]);
            // Astah names a transition after its whole label.
            let words = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
            if words(&data.name) == words(&props.label()) {
                data.name.clear();
            }
            self.create(id, data, Some(transition));
        }
        if let Some(diagram) = r.obj(machine, "diagram") {
            self.diagrams.push((id, diagram));
        }
    }

    /// The substates of `composite`, owned by `owner`, each with its own substates.
    fn vertices(&mut self, owner: Id, composite: ObjId) {
        let r = self.r;
        for vertex in r.list(composite, "subvertex") {
            // The substates of an orthogonal state are its regions, composite states of their
            // own; Astah's API names them Region0, Region1 …
            if r.flag(composite, "isOrthogonal") {
                let mut data = self.data(vertex, ElementKind::Region, &[]);
                if is_default_region_name(&data.name) {
                    data.name.clear();
                }
                if let Some(region) = self.create(owner, data, Some(vertex)) {
                    self.vertices(region, vertex);
                }
                continue;
            }
            let action = |field| r.obj(vertex, field).map(|a| r.name(a)).unwrap_or_default();
            let kind = match r.class(vertex) {
                "UCompositeStateImp" | "UStateImp" | "USimpleStateImp" => {
                    ElementKind::State(StateProps {
                        entry: action("entry"),
                        do_activity: action("doActivity"),
                        exit: action("exit"),
                    })
                }
                "UFinalStateImp" => ElementKind::FinalState,
                "UPseudostateImp" => {
                    let pseudostate = match r.text(vertex, "kind").as_str() {
                        "initial" => PseudostateKind::Initial,
                        "choice" => PseudostateKind::Choice,
                        "junction" => PseudostateKind::Junction,
                        "fork" => PseudostateKind::Fork,
                        "join" => PseudostateKind::Join,
                        "shallowHistory" => PseudostateKind::ShallowHistory,
                        "deepHistory" => PseudostateKind::DeepHistory,
                        _ => {
                            self.skip(vertex);
                            continue;
                        }
                    };
                    ElementKind::pseudostate(pseudostate)
                }
                _ => {
                    self.skip(vertex);
                    continue;
                }
            };
            let mut data = self.data(vertex, kind, &[]);
            if !matches!(data.kind, ElementKind::State(_)) && is_default_node_name(&data.name) {
                data.name.clear();
            }
            let Some(id) = self.create(owner, data, Some(vertex)) else { continue };
            for transition in r.list(vertex, "internalTransitions") {
                let named = |field| r.obj(transition, field).map(|o| r.name(o)).unwrap_or_default();
                let props = InternalTransitionProps {
                    trigger: named("trigger"),
                    guard: named("guard"),
                    effect: named("effect"),
                };
                let mut data =
                    self.data(transition, ElementKind::InternalTransition(props.clone()), &[]);
                // Astah names it after its label, which UmlCraft shows instead of a name.
                data.name.clear();
                self.create(id, data, Some(transition));
            }
            self.vertices(id, vertex);
        }
    }

    fn partition(&mut self, owner: Id, partition: ObjId) {
        // A damaged file may list a partition among its own lanes.
        if !self.dimensions.insert(partition) {
            return;
        }
        // Astah puts every set of lanes into a "dimension" of its own, drawn as a header above
        // them. Unless the user named it, it is left out and its lanes take its place.
        let is_dimension =
            self.model.get(owner).is_some_and(|e| matches!(e.kind(), ElementKind::Activity))
                && is_default_dimension_name(&self.r.name(partition));
        if is_dimension {
            for sub in self.r.list(partition, "subGroup") {
                self.partition(owner, sub);
            }
            return;
        }
        let data = self.data(partition, ElementKind::Partition, &[]);
        let Some(id) = self.create(owner, data, Some(partition)) else { return };
        for sub in self.r.list(partition, "subGroup") {
            self.partition(id, sub);
        }
    }

    fn activity_node_kind(&self, vertex: ObjId) -> Option<ActivityNodeKind> {
        Some(match self.r.class(vertex) {
            // A subactivity calls another activity; UmlCraft shows it as an action.
            "UActionStateImp" | "USubactivityStateImp" => ActivityNodeKind::Action,
            "UObjectFlowStateImp" => ActivityNodeKind::Object,
            "UFinalStateImp" => ActivityNodeKind::ActivityFinal,
            "UFlowFinalNodeImp" => ActivityNodeKind::FlowFinal,
            "UPseudostateImp" => match self.r.text(vertex, "kind").as_str() {
                "initial" => ActivityNodeKind::Initial,
                "fork" => ActivityNodeKind::Fork,
                "join" => ActivityNodeKind::Join,
                // Astah draws one "decision and merge" node; its flows tell which it is.
                "choice" | "junction" => {
                    let count = |field| self.r.list(vertex, field).len();
                    if count("incoming") > 1 && count("outgoing") <= 1 {
                        ActivityNodeKind::Merge
                    } else {
                        ActivityNodeKind::Decision
                    }
                }
                _ => return None,
            },
            _ => return None,
        })
    }

    fn diagram(&mut self, owner: Id, object: ObjId) {
        let kind = match (self.r.class(object), self.r.text(object, "type").as_str()) {
            ("USequenceDiagramImp", _) => DiagramKind::Sequence,
            ("UActivityDiagramImp", _) => DiagramKind::Activity,
            ("UStateChartDiagramImp", _) => DiagramKind::StateMachine,
            (_, "Class Diagram") => DiagramKind::Class,
            (_, "UseCase Diagram") => DiagramKind::UseCase,
            (_, "Component Diagram") => DiagramKind::Component,
            (_, "Deployment Diagram") => DiagramKind::Deployment,
            _ => {
                self.skip(object);
                return;
            }
        };
        let mut owner = owner;
        if kind == DiagramKind::Sequence {
            match self.interaction(owner, object) {
                Some(interaction) => owner = interaction,
                None => {
                    self.skip(object);
                    return;
                }
            }
        }
        let mut data = self.data(object, ElementKind::diagram(kind), &[]);
        if let ElementKind::Diagram(props) = &mut data.kind {
            let off = |field| self.r.g.bool_field(object, field) == Some(false);
            props.hide_message_numbers = kind == DiagramKind::Sequence && off("indexVisible");
            props.hide_return_values = kind == DiagramKind::Sequence && off("ReturnValueVisible");
            props.flat_message_numbers = kind == DiagramKind::Sequence
                && self.r.g.bool_field(object, "flatIndex") == Some(true);
            props.hide_frame = kind == DiagramKind::Sequence
                && self.r.list(object, "presentation").into_iter().any(|p| {
                    self.r.class(p) == "FramePresentation"
                        && self.r.g.bool_field(p, "visibility") == Some(false)
                });
        }
        let Some(diagram) = self.create(owner, data, Some(object)) else { return };
        self.presentations(diagram, kind, object);
    }

    /// Creates the interaction behind a sequence diagram: lifelines, then messages and
    /// combined fragments from top to bottom.
    fn interaction(&mut self, owner: Id, diagram: ObjId) -> Option<Id> {
        let r = self.r;
        let collaboration = r.obj(diagram, "collaboration")?;
        let source = *r.list(collaboration, "interactions").first()?;
        let data = ElementData::new(r.name(diagram), ElementKind::Interaction);
        let interaction = self.create(owner, data, Some(source))?;

        for ownership in self.r.list(collaboration, "ownedElementOwnership") {
            let Some(role) = self.r.obj(ownership, "ownedElement") else { continue };
            if self.r.class(role) != "UClassifierRoleImp" {
                continue;
            }
            let represents = self.r.obj(role, "base").and_then(|b| self.classifier(b));
            let data = self.data(role, ElementKind::Lifeline(LifelineProps { represents }), &[]);
            self.create(interaction, data, Some(role));
        }

        // The interaction lists every message and fragment; those inside an operand belong to
        // the operand.
        let mut nested = HashSet::new();
        let mut visited = HashSet::new();
        let mut stack = self.r.list(source, "fragments");
        while let Some(fragment) = stack.pop() {
            // A damaged file may nest a fragment in itself.
            if !visited.insert(fragment) {
                continue;
            }
            for operand in self.r.list(fragment, "operands") {
                nested.extend(self.r.list(operand, "messages"));
                nested.extend(self.r.list(operand, "fragments"));
                stack.extend(self.r.list(operand, "fragments"));
            }
        }
        let top_level = |list: Vec<ObjId>| -> Vec<ObjId> {
            list.into_iter().filter(|m| !nested.contains(m)).collect()
        };
        let messages = top_level(self.r.list(source, "message"));
        let fragments = top_level(self.r.list(source, "fragments"));
        // A reply points to its call as `activator`. The interaction lists all messages.
        for message in self.r.list(source, "message") {
            if action_type(self.r, message) == Some(REPLY)
                && let Some(call) = self.r.obj(message, "activator")
            {
                self.replied.insert(call);
            }
        }
        self.interaction_items(interaction, messages, fragments);
        Some(interaction)
    }

    /// Vertical position of a message or fragment on its diagram, for ordering.
    fn top(&self, object: ObjId) -> f32 {
        let r = self.r;
        r.list(object, "presentation")
            .first()
            .and_then(|&p| {
                r.points(p, "allPoints")
                    .first()
                    .map(|pt| pt.y)
                    .or_else(|| r.obj(p, "location").and_then(|l| r.point(l)).map(|pt| pt.y))
            })
            .unwrap_or(f32::MAX)
    }

    fn interaction_items(&mut self, container: Id, messages: Vec<ObjId>, fragments: Vec<ObjId>) {
        let mut items: Vec<(f32, bool, ObjId)> = messages
            .into_iter()
            .map(|m| (self.top(m), true, m))
            .chain(fragments.into_iter().map(|f| (self.top(f), false, f)))
            .collect();
        items.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, is_message, object) in items {
            if is_message {
                self.message(container, object);
            } else {
                self.fragment(container, object);
            }
        }
    }

    fn message(&mut self, container: Id, object: ObjId) {
        let r = self.r;
        let lifeline = |field| {
            r.obj(object, field).and_then(|o| self.ids.get(&o).copied()).filter(|id| {
                matches!(self.model.get(*id).map(|e| e.kind()), Some(ElementKind::Lifeline(_)))
            })
        };
        // Found and lost messages start or end at a gate instead of a lifeline; Astah's API
        // leaves that end empty instead.
        let is_gate = |field| r.obj(object, field).is_none_or(|o| r.class(o) == "UGateImp");
        let (from, to, gate) = match (lifeline("sender"), lifeline("receiver")) {
            (Some(from), Some(to)) => (from, to, MessageGate::None),
            (None, Some(to)) if is_gate("sender") => (to, to, MessageGate::Found),
            (Some(from), None) if is_gate("receiver") => (from, from, MessageGate::Lost),
            _ => {
                self.skip(object);
                return;
            }
        };
        let action = r.obj(object, "action");
        let asynchronous = action.is_some_and(|a| r.flag(a, "asynchronous"));
        let sort = match action_type(r, object) {
            Some(CREATE) => MessageSort::Create,
            Some(REPLY) => MessageSort::Reply,
            Some(DESTROY) => MessageSort::Destroy,
            _ if asynchronous => MessageSort::Asynchronous,
            _ => MessageSort::Synchronous,
        };
        // Astah keeps the return value on the call; UmlCraft shows it on the reply.
        let return_value = match sort {
            MessageSort::Reply => Some(r.text(object, "returnValue"))
                .filter(|v| !v.is_empty())
                .or_else(|| r.obj(object, "activator").map(|call| r.text(call, "returnValue")))
                .unwrap_or_default(),
            _ if self.replied.contains(&object) => String::new(),
            _ => r.text(object, "returnValue"),
        };
        let arguments = action
            .map(|a| r.list(a, "actualArgument"))
            .unwrap_or_default()
            .into_iter()
            .map(|argument| r.text(argument, "value"))
            .collect::<Vec<_>>()
            .join(", ");
        let operation = action
            .and_then(|a| r.obj(a, "operation"))
            .and_then(|o| self.ids.get(&o).copied())
            .filter(|id| {
                matches!(self.model.get(*id).map(|e| e.kind()), Some(ElementKind::Operation(_)))
            });
        let guard = r.obj(object, "guard").map(|g| r.name(g)).unwrap_or_default();
        let kind = ElementKind::Message(MessageProps {
            sort,
            from,
            to,
            gate,
            operation,
            arguments,
            return_value,
            guard,
        });
        let data = self.data(object, kind, &[]);
        self.create(container, data, Some(object));
    }

    fn fragment(&mut self, container: Id, object: ObjId) {
        let r = self.r;
        let covered =
            r.list(object, "covereds").iter().filter_map(|c| self.ids.get(c).copied()).collect();
        if r.class(object) == "UInteractionUseImp" {
            // The interaction it refers to may not exist yet; see `resolve_references`.
            let arguments = r.text(object, "argument");
            let kind = ElementKind::InteractionUse(InteractionUseProps {
                refers_to: None,
                arguments,
                covered,
            });
            let data = self.data(object, kind, &[]);
            if let Some(id) = self.create(container, data, Some(object))
                && let Some(target) = r.obj(object, "refersTo")
            {
                self.references.push((id, target));
            }
            return;
        }
        if r.class(object) != "UCombinedFragmentImp" {
            self.skip(object);
            return;
        }
        let operator = match r.text(object, "operator").as_str() {
            "opt" => InteractionOperator::Opt,
            "loop" => InteractionOperator::Loop,
            "par" => InteractionOperator::Par,
            "break" => InteractionOperator::Break,
            "critical" => InteractionOperator::Critical,
            "neg" => InteractionOperator::Neg,
            "assert" => InteractionOperator::Assert,
            "strict" => InteractionOperator::Strict,
            "seq" => InteractionOperator::Seq,
            "ignore" => InteractionOperator::Ignore,
            "consider" => InteractionOperator::Consider,
            _ => InteractionOperator::Alt,
        };
        let kind = ElementKind::CombinedFragment(CombinedFragmentProps { operator, covered });
        let data = self.data(object, kind, &[]);
        let Some(fragment) = self.create(container, data, Some(object)) else { return };
        for operand in self.r.list(object, "operands") {
            let guard =
                self.r.obj(operand, "guard").map(|g| self.r.text(g, "body")).unwrap_or_default();
            let kind = ElementKind::InteractionOperand(InteractionOperandProps { guard });
            let data = self.data(operand, kind, &[]);
            let Some(id) = self.create(fragment, data, Some(operand)) else { continue };
            let messages = self.r.list(operand, "messages");
            let fragments = self.r.list(operand, "fragments");
            self.interaction_items(id, messages, fragments);
        }
    }

    /// The diagram's presentations from back to front; a greater depth lies further back.
    fn presentations_back_to_front(&self, object: ObjId) -> Vec<ObjId> {
        let mut sources = self.r.list(object, "presentation");
        let depth = |p: ObjId| self.r.g.field(p, "depth").and_then(Value::as_i64).unwrap_or(0);
        sources.sort_by_key(|&p| std::cmp::Reverse(depth(p)));
        sources
    }

    /// Location and size of a node presentation as stored.
    fn rect(&self, p: ObjId) -> Option<Rect> {
        let r = self.r;
        let at = r.obj(p, "location").and_then(|l| r.point(l))?;
        Some(Rect::new(at.x, at.y, r.num(p, "width")? as f32, r.num(p, "height")? as f32))
    }

    /// Adds a presentation, at the back or in front of everything so far.
    fn insert(&mut self, diagram: Id, presentation: Presentation, back: bool) -> Option<Id> {
        let index = if back { 0 } else { self.model.view(diagram)?.presentations().len() };
        let id = presentation.id;
        if self.model.apply(Op::InsertPresentation { diagram, index, presentation }).is_err() {
            self.lost_presentations += 1;
            return None;
        }
        Some(id)
    }

    fn fill(&self, p: ObjId) -> Style {
        let mut style = Style::default();
        if let Some(fill) =
            self.r.style(p, "fill.color").filter(|c| !c.eq_ignore_ascii_case(DEFAULT_FILL))
        {
            style.fill = fill.to_ascii_lowercase().parse().ok();
        }
        // Astah stores a font size of its own with `font.size.auto` false.
        style.font_size = self
            .r
            .style(p, "font.size")
            .filter(|_| self.r.style(p, "font.size.auto").as_deref() == Some("false"))
            .and_then(|s| s.parse::<f32>().ok())
            .filter(|&size| size.is_finite() && size > 0.0 && size != scene::FONT_SIZE);
        style
    }

    /// Recreates a sequence diagram with Astah's geometry: the frame, lifelines with their
    /// activation bars, messages, fragments and notes. Everything is moved by the same amount
    /// as on other diagrams, so that the content starts at the margin.
    fn sequence_presentations(&mut self, diagram: Id, object: ObjId) {
        let r = self.r;
        let sources = self.presentations_back_to_front(object);
        let class = |p: ObjId| r.class(p);
        let is_node = |p: &ObjId| {
            !matches!(class(*p), "LabelPresentation" | "MessagePresentation")
                && r.obj(*p, "allPoints").is_none()
        };
        let min = sources
            .iter()
            .filter(|p| is_node(p))
            .filter_map(|&p| self.rect(p))
            .fold((f32::MAX, f32::MAX), |(x, y), b| (x.min(b.x), y.min(b.y)));
        let shift = Point::new((MARGIN - min.0).max(0.0), (MARGIN - min.1).max(0.0));
        let rect = |this: &Self, p: ObjId| this.rect(p).map(|b| b.translate(shift.x, shift.y));
        let element =
            |this: &Self, p: ObjId| r.obj(p, "model").and_then(|m| this.ids.get(&m)).copied();
        let mut presentation_of: HashMap<ObjId, Id> = HashMap::new();

        let destroyed: HashSet<ObjId> = sources
            .iter()
            .filter(|&&p| class(p) == "TerminationPresentation")
            .filter_map(|&p| r.list(p, "servers").first().copied())
            .collect();
        // Nodes in Astah's z-order; bars and messages lie in front of them.
        for &p in &sources {
            if class(p) == "NotePresentation" {
                if let Some((_, node)) = self.node(diagram, DiagramKind::Sequence, p) {
                    let bounds = node.bounds.translate(shift.x, shift.y);
                    let mut presentation = Presentation::node(node.element, bounds);
                    presentation.style = node.style;
                    if let Some(id) = self.insert(diagram, presentation, false) {
                        presentation_of.insert(p, id);
                    }
                }
                continue;
            }
            let Some(bounds) = rect(self, p) else { continue };
            let presentation = match class(p) {
                "FramePresentation" => Presentation::node(Some(diagram), bounds),
                "ClassifierRolePresentation" => {
                    let Some(lifeline) = element(self, p).filter(|&e| {
                        matches!(
                            self.model.get(e).map(|e| e.kind()),
                            Some(ElementKind::Lifeline(_))
                        )
                    }) else {
                        continue;
                    };
                    let length = r.num(p, "lifelineLength").unwrap_or(0.0) as f32;
                    let mut presentation = Presentation::lifeline(lifeline, bounds, length);
                    if let Shape::Lifeline { destroyed: d, .. } = &mut presentation.shape {
                        *d = destroyed.contains(&p);
                    }
                    presentation.style = self.fill(p);
                    presentation.style.iconic =
                        r.g.field(p, "notationType").and_then(Value::as_i64) == Some(1);
                    let off = |field: &str| r.g.bool_field(p, field) == Some(false);
                    presentation.style.hide_instance_name = off("nameVisibility");
                    presentation.style.hide_instance_type = off("baseVisibility");
                    presentation
                }
                "CombinedFragmentPresentation" | "InteractionUsePresentation" => {
                    let Some(e) = element(self, p) else { continue };
                    self.fragment_presentation(p, e, bounds)
                }
                _ => continue,
            };
            // The diagram frame is always behind everything.
            let back = presentation.element == Some(diagram);
            if let Some(id) = self.insert(diagram, presentation, back) {
                presentation_of.insert(p, id);
            }
        }

        // Activation bars hang on their lifeline; a gate is an activation of the frame.
        let lane_of = |this: &Self, server: ObjId, presentation_of: &HashMap<ObjId, Id>| {
            let lifeline = match class(server) {
                "ActivationPresentation" => *r.list(server, "servers").first()?,
                _ => server,
            };
            presentation_of.get(&lifeline).copied().filter(|&id| {
                this.model
                    .view(diagram)
                    .and_then(|v| v.get(id))
                    .is_some_and(|l| matches!(l.shape, Shape::Lifeline { .. }))
            })
        };
        for &p in &sources {
            if class(p) != "ActivationPresentation"
                || r.obj(p, "model").is_some_and(|m| class(m) == "UGateImp")
            {
                continue;
            }
            let Some(bounds) = rect(self, p) else { continue };
            let Some(&server) = r.list(p, "servers").first() else { continue };
            let Some(lifeline) = lane_of(self, server, &presentation_of) else { continue };
            let mut presentation = Presentation::activation(lifeline, bounds.y, bounds.height);
            // Astah keeps bars of width 0 and draws them as a line.
            if let Shape::Activation { width, .. } = &mut presentation.shape {
                *width = (bounds.width.abs() < 1e-3).then_some(0.0);
            }
            presentation.style = self.fill(p);
            if let Some(id) = self.insert(diagram, presentation, false) {
                presentation_of.insert(p, id);
            }
        }

        for &p in &sources {
            if class(p) == "MessagePresentation" {
                self.message_presentation(diagram, p, shift, |this, s| {
                    lane_of(this, s, &presentation_of)
                });
            }
        }
        for &p in &sources {
            if class(p) == "NoteAnchorPresentation" {
                self.edge(diagram, p, &presentation_of, shift);
            }
        }
        // Astah draws free texts and lines in front of the bars.
        for &p in &sources {
            if let Some(presentation) = self.free_shape(p, shift) {
                self.insert(diagram, presentation, false);
            }
        }
    }

    /// A combined fragment with the offsets of its operands and its guards placed by hand, or
    /// an interaction use.
    fn fragment_presentation(&self, p: ObjId, element: Id, bounds: Rect) -> Presentation {
        let r = self.r;
        let mut presentation = Presentation::fragment(element, bounds);
        if r.class(p) == "InteractionUsePresentation" {
            presentation.shape = Shape::Node { bounds, regions: None };
        } else if let Shape::Fragment { separators, guards, .. } = &mut presentation.shape {
            *separators = r
                .list(p, "operandOffsets")
                .iter()
                .filter_map(|&o| r.num(o, "value"))
                .map(|o| o as f32)
                .collect();
            *guards = r
                .list(p, "guardPresentations")
                .iter()
                .map(|&g| {
                    r.obj(g, "localMovement")
                        .and_then(|l| r.point(l))
                        .filter(|m| m.x.abs() > 1e-3 || m.y.abs() > 1e-3)
                })
                .collect();
            if guards.iter().all(Option::is_none) {
                guards.clear();
            }
        }
        presentation.style = self.fill(p);
        presentation
    }

    /// A message between the lifelines of its ends; `lane` finds the lifeline presentation
    /// behind one of Astah's servers (a lifeline or an activation bar).
    fn message_presentation(
        &mut self,
        diagram: Id,
        p: ObjId,
        shift: Point,
        lane: impl Fn(&Self, ObjId) -> Option<Id>,
    ) {
        let r = self.r;
        let Some(element) = r.obj(p, "model").and_then(|m| self.ids.get(&m)).copied() else {
            return;
        };
        let Some(ElementKind::Message(props)) = self.model.get(element).map(|e| e.kind()) else {
            return;
        };
        let (gate, props_sort) = (props.gate, props.sort);
        let points: Vec<Point> =
            r.points(p, "allPoints").into_iter().map(|pt| pt.offset(shift.x, shift.y)).collect();
        let (Some(&first), Some(&last)) = (points.first(), points.last()) else { return };
        let servers = r.list(p, "servers");
        let ends: Vec<Option<Id>> = servers.iter().map(|&s| lane(self, s)).collect();
        let (source, target, gate_x) =
            match (gate, ends.first().copied().flatten(), ends.get(1).copied().flatten()) {
                (MessageGate::None, Some(s), Some(t)) => (s, t, None),
                (MessageGate::Found, _, Some(t)) => (t, t, Some(first.x)),
                (MessageGate::Lost, Some(s), _) => (s, s, Some(last.x)),
                _ => return,
            };
        let center = self
            .model
            .view(diagram)
            .and_then(|v| v.get(source))
            .and_then(|l| l.bounds())
            .map_or(first.x, |head| head.center().x);
        let mut presentation = Presentation::message(element, source, target, first.y);
        if let Shape::Message { end_y, gate_x: g, loop_x, labels, .. } = &mut presentation.shape {
            if (last.y - first.y).abs() > 1e-3 {
                *end_y = Some(last.y);
            }
            *g = gate_x;
            // A loop to self dragged wider or narrower than Astah's `SELF_LOOP` beyond its start.
            if source == target && gate_x.is_none() && points.len() == 4 {
                let width = points[1].x - points[0].x;
                if (width - SELF_LOOP).abs() > 0.5 {
                    *loop_x = Some(points[1].x - center);
                }
            }
            // A label moved by hand, relative to the sender's lifeline at the message's height.
            // Astah draws it `localMovement` away from its own place: across from the stored
            // `location`, which older versions left stale in height, and up from the label's
            // place above the line.
            labels.name = r.obj(p, "namePresentation").and_then(|label| {
                let moved = r.obj(label, "localMovement").and_then(|l| r.point(l))?;
                if moved.x.abs() < 1e-3 && moved.y.abs() < 1e-3 {
                    return None;
                }
                let at = r.obj(label, "location").and_then(|l| r.point(l))?;
                // Astah centers the text in the stored box by Java's whole-pixel width; the box
                // is as wide as the exact width (newer files) or the whole-pixel one (older).
                // Labels broken into several lines are left as they are.
                let text = r.text(label, "label");
                let one_line = r.num(label, "height") < Some(20.0);
                let stored = r.num(label, "width").filter(|_| one_line).map(|w| w as f32);
                let java = umlcraft_notation::metrics::java_width(&text, scene::FONT_SIZE, false);
                let inset = stored.zip(java).map_or(0.0, |(w, j)| (w - j) / 2.0);
                Some(Point::new(
                    at.x + moved.x + inset + shift.x - center,
                    moved.y - umlcraft_notation::sequence::LABEL_ABOVE,
                ))
            });
        }
        presentation.style = self.fill(p);
        // Astah hides the parentheses of every reply; only for calls it is a choice.
        let flag = |name| r.g.bool_field(p, name);
        let call = props_sort != MessageSort::Reply;
        presentation.style.hide_parameters = call && flag("parameterVisibility") == Some(false);
        presentation.style.hide_parameter_types =
            call && flag("parameterTypeVisibility") == Some(false);
        presentation.style.show_return_type = call && flag("returnValueVisibility") == Some(true);
        self.insert(diagram, presentation, false);
    }

    /// Recreates the diagram's nodes and edges.
    fn presentations(&mut self, diagram: Id, kind: DiagramKind, object: ObjId) {
        if kind == DiagramKind::Sequence {
            self.sequence_presentations(diagram, object);
            return;
        }
        let sources = self.presentations_back_to_front(object);
        let (edges, nodes): (Vec<ObjId>, Vec<ObjId>) = sources.into_iter().partition(|&p| {
            self.r.obj(p, "allPoints").is_some() || self.r.class(p) == "NoteAnchorPresentation"
        });

        // Texts that name a system boundary are part of it.
        let names: HashSet<ObjId> = if kind == DiagramKind::UseCase {
            nodes
                .iter()
                .filter(|&&p| self.r.class(p) == "RectPresentation")
                .filter_map(|&p| self.boundary_text(p).map(|(t, _)| t))
                .collect()
        } else {
            HashSet::new()
        };
        // Nodes, and free texts and lines (without node data) among them in Astah's z-order.
        let mut placed: Vec<(ObjId, Option<NodeData>)> = Vec::new();
        for p in nodes {
            if matches!(self.r.class(p), "TextPresentation" | "LinePresentation") {
                if !names.contains(&p) {
                    placed.push((p, None));
                }
            } else if let Some((p, node)) = self.node(diagram, kind, p) {
                placed.push((p, Some(node)));
            }
        }
        let min = placed
            .iter()
            .filter_map(|(p, n)| n.as_ref().map(|n| n.bounds).or_else(|| self.rect(*p)))
            .fold((f32::MAX, f32::MAX), |(x, y), b| (x.min(b.x), y.min(b.y)));
        let shift = Point::new((MARGIN - min.0).max(0.0), (MARGIN - min.1).max(0.0));

        let mut presentation_of = HashMap::new();
        for (source, node) in placed {
            let Some(mut node) = node else {
                if let Some(presentation) = self.free_shape(source, shift) {
                    self.insert(diagram, presentation, false);
                }
                continue;
            };
            node.bounds.x += shift.x;
            node.bounds.y += shift.y;
            // Containers and fragments go to the back, as when they are drawn in UmlCraft.
            let container = node.element.and_then(|e| self.model.get(e)).is_some_and(|e| {
                palette::is_container(e.kind())
                    || matches!(e.kind(), ElementKind::CombinedFragment(_))
            });
            let presentation = Presentation {
                id: Id::generate(),
                element: node.element,
                shape: Shape::Node { bounds: node.bounds, regions: node.regions },
                style: node.style,
            };
            let index = if container {
                0
            } else {
                self.model.view(diagram).map_or(0, |v| v.presentations().len())
            };
            let id = presentation.id;
            if self.model.apply(Op::InsertPresentation { diagram, index, presentation }).is_ok() {
                presentation_of.insert(source, id);
            }
        }
        // Note anchors come last: they may end on another line.
        let (anchors, lines): (Vec<ObjId>, Vec<ObjId>) =
            edges.into_iter().partition(|&p| self.r.class(p) == "NoteAnchorPresentation");
        for p in lines.into_iter().chain(anchors) {
            if let Some(id) = self.edge(diagram, p, &presentation_of, shift) {
                presentation_of.insert(p, id);
            }
        }
    }

    fn node(&mut self, diagram: Id, kind: DiagramKind, p: ObjId) -> Option<(ObjId, NodeData)> {
        let r = self.r;
        let class = r.class(p);
        let element = match class {
            // The frame of a diagram other than a sequence diagram, when shown, is a node
            // presentation of the diagram.
            "FramePresentation"
                if kind != DiagramKind::Sequence
                    && r.g.bool_field(p, "visibility") == Some(true) =>
            {
                diagram
            }
            // Header cells belong to Astah's tables and point at the diagram too.
            "FramePresentation" | "LabelPresentation" | "HeaderCellPresentation" => return None,
            "NotePresentation" => self.note(diagram, p)?,
            "RectPresentation" if kind == DiagramKind::UseCase => {
                self.system_boundary(diagram, p)?
            }
            _ => {
                let model = r.obj(p, "model")?;
                match self.ids.get(&model) {
                    Some(&id) => id,
                    None => return None,
                }
            }
        };
        let element_kind = self.model.get(element)?.kind().clone();
        if element != diagram && !palette::can_show(kind, &element_kind) {
            return None;
        }
        let r = self.r;
        let origin = r.obj(p, "location").and_then(|l| r.point(l)).unwrap_or_default();
        let default = palette::default_size_for(&element_kind);
        let width = r.num(p, "width").filter(|w| *w > 0.0).map_or(default.width, |w| w as f32);
        let height = r.num(p, "height").filter(|h| *h > 0.0).map_or(default.height, |h| h as f32);
        let mut style = self.fill(p);
        if class == "ClassifierPresentation" {
            style.hide_attributes = r.g.bool_field(p, "allAttributeVisibility") == Some(false);
            style.hide_operations = r.g.bool_field(p, "allOperationVisibility") == Some(false);
            style.iconic = matches!(element_kind, ElementKind::Interface(_))
                && r.g.field(p, "notationType").and_then(Value::as_i64) == Some(1);
            style.auto_size = scene::is_boxed_classifier(&element_kind)
                && r.g.bool_field(p, "doAutoResize") == Some(true);
        }
        if class == "RectPresentation" {
            style.name_offset = self.boundary_name(p).map(|(_, at)| at);
        }
        // Astah never breaks a use case's name at spaces; it widens the ellipse instead.
        if class == "UseCasePresentation" {
            style.auto_size = true;
        }
        // Instances may hide their name or classifier, objects their slots.
        if matches!(
            class,
            "ObjectClassPresentation"
                | "ComponentInstancePresentation"
                | "NodeInstancePresentation"
        ) {
            let off = |field: &str| r.g.bool_field(p, field) == Some(false);
            style.hide_instance_name = off("instanceNameVisibility");
            style.hide_instance_type = off("instanceTypeVisibility");
            style.hide_attributes =
                class == "ObjectClassPresentation" && off("allAttributeVisibility");
            style.hide_empty_slots =
                class == "ObjectClassPresentation" && off("noValueAttributeVisibility");
            style.auto_size = class == "ObjectClassPresentation"
                && r.g.bool_field(p, "doAutoResize") == Some(true);
        }
        // Orthogonal states keep where the lines between their regions go, as distances from
        // their top edge (regions one below the other) or left edge (side by side).
        let has_regions =
            self.model.children(element).iter().any(|&c| {
                self.model.get(c).is_some_and(|c| matches!(c.kind(), ElementKind::Region))
            });
        let regions = has_regions.then(|| {
            let (stacked, side) =
                (r.numbers(p, "horizontalRegionOffsets"), r.numbers(p, "verticalRegionOffsets"));
            let vertical = stacked.is_empty() && !side.is_empty();
            let offsets = if vertical { side } else { stacked };
            Regions { vertical, separators: offsets.into_iter().map(|o| o as f32).collect() }
        });
        Some((
            p,
            NodeData {
                element: Some(element),
                bounds: Rect::new(origin.x, origin.y, width, height),
                style,
                regions,
            },
        ))
    }

    /// The comment shown by a note, created on first use next to the diagram.
    fn note(&mut self, diagram: Id, p: ObjId) -> Option<Id> {
        let comment = self.r.obj(p, "model")?;
        if let Some(&id) = self.ids.get(&comment) {
            return Some(id);
        }
        // Astah lists the package a note belongs to among its annotated elements. Packages that
        // a note really annotates are connected by an anchor, which `edge` adds.
        let (owners, annotated): (Vec<Id>, Vec<Id>) = self
            .r
            .list(comment, "annotatedElement")
            .iter()
            .filter_map(|a| self.ids.get(a).copied())
            .partition(|&id| {
                matches!(
                    self.model.get(id).map(|e| e.kind()),
                    Some(ElementKind::Package | ElementKind::Model)
                )
            });
        let body = self.r.text(comment, "body");
        let data = self.data(comment, ElementKind::Comment(CommentProps { body, annotated }), &[]);
        let owner = match owners.first() {
            Some(&package) => package,
            None => self.model.get(diagram)?.owner()?,
        };
        self.create(owner, data, Some(comment))
    }

    /// Astah has no system boundary: on a use case diagram it is a rectangle, and a text at its
    /// top edge gives the name.
    fn system_boundary(&mut self, diagram: Id, p: ObjId) -> Option<Id> {
        let name = self.boundary_name(p).map(|(text, _)| text).unwrap_or_default();
        let owner = self.model.get(diagram)?.owner()?;
        self.create(owner, ElementData::new(name, ElementKind::SystemBoundary), None)
    }

    /// The text that names the rectangle `p`, and where its box lies relative to the
    /// rectangle's top left: the topmost text over the rectangle that starts at most 40 above
    /// or below its top.
    fn boundary_name(&self, p: ObjId) -> Option<(String, Point)> {
        let (t, at) = self.boundary_text(p)?;
        Some((self.r.text(t, "text"), at))
    }

    /// The text presentation that names the rectangle `p`, see [`Self::boundary_name`].
    fn boundary_text(&self, p: ObjId) -> Option<(ObjId, Point)> {
        const NAME_ZONE: f32 = 40.0;
        let rect = self.rect(p)?;
        let texts = self.r.obj(p, "diagram").map(|d| self.r.list(d, "presentation"));
        texts
            .unwrap_or_default()
            .into_iter()
            .filter(|&t| self.r.class(t) == "TextPresentation")
            .filter_map(|t| Some((t, self.rect(t)?)))
            .filter(|(_, b)| {
                (rect.x..=rect.right()).contains(&b.center().x) && (b.y - rect.y).abs() < NAME_ZONE
            })
            .min_by(|(_, a), (_, b)| a.y.total_cmp(&b.y))
            .map(|(t, b)| (t, Point::new(b.x - rect.x, b.y - rect.y)))
    }

    /// A free text or line, moved by `shift`. A line runs from its location by its
    /// `localMovement`; `line.type` dashes it, `line.arrow.type` puts arrowheads on it.
    fn free_shape(&self, p: ObjId, shift: Point) -> Option<Presentation> {
        let r = self.r;
        match r.class(p) {
            "TextPresentation" => {
                let bounds = self.rect(p)?.translate(shift.x, shift.y);
                let mut presentation = Presentation::text(bounds, r.text(p, "text"));
                presentation.style.font_size = self.fill(p).font_size;
                Some(presentation)
            }
            "LinePresentation" => {
                let from = r.obj(p, "location").and_then(|l| r.point(l))?.offset(shift.x, shift.y);
                let by = r.obj(p, "localMovement").and_then(|l| r.point(l)).unwrap_or_default();
                let mut presentation = Presentation::line(from, from.offset(by.x, by.y));
                if let Shape::Line { dashed, start_arrow, end_arrow, .. } = &mut presentation.shape
                {
                    let line = r.style(p, "line.type").unwrap_or_default();
                    *dashed = line.starts_with("dash") || line.starts_with("dot");
                    let arrows = r.style(p, "line.arrow.type").unwrap_or_default();
                    *start_arrow = arrows.contains("start") || arrows.contains("both");
                    *end_arrow = arrows.contains("end") || arrows.contains("both");
                }
                Some(presentation)
            }
            _ => None,
        }
    }

    /// Adds the edge presentation `p`, between presentations added before; returns its id.
    fn edge(
        &mut self,
        diagram: Id,
        p: ObjId,
        presentation_of: &HashMap<ObjId, Id>,
        shift: Point,
    ) -> Option<Id> {
        let r = self.r;
        let ends: Vec<Id> =
            r.list(p, "servers").iter().filter_map(|s| presentation_of.get(s).copied()).collect();
        let [mut source, mut target] = ends[..] else { return None };
        // Astah's "source" end and the A labels belong to the first server.
        let mut swapped = false;
        let element_of = |presentation: Id| {
            self.model.view(diagram).and_then(|v| v.get(presentation)).and_then(|p| p.element)
        };
        let element = if r.class(p) == "NoteAnchorPresentation" {
            // Anchors run from the note to the annotated element.
            let is_note = |id: Id| {
                element_of(id)
                    .and_then(|e| self.model.get(e))
                    .is_some_and(|e| matches!(e.kind(), ElementKind::Comment(_)))
            };
            if !is_note(source) && is_note(target) {
                std::mem::swap(&mut source, &mut target);
                swapped = true;
            }
            if let (Some(comment), Some(annotated)) = (element_of(source), element_of(target)) {
                self.annotate(comment, annotated);
            }
            None
        } else if r.class(p) == "ContainmentPresentation" {
            // A containment line has no model of its own; it stands for the nested element and
            // runs from its owner.
            let (a, b) = (element_of(source)?, element_of(target)?);
            let owner = |e: Id| self.model.get(e).and_then(|e| e.owner());
            if owner(a) == Some(b) {
                std::mem::swap(&mut source, &mut target);
                swapped = true;
                Some(a)
            } else {
                (owner(b) == Some(a)).then_some(b)?;
                Some(b)
            }
        } else {
            let &element = r.obj(p, "model").and_then(|m| self.ids.get(&m))?;
            // Message edges are added from the model, see `message_edges`.
            if matches!(self.model.get(element).map(|e| e.kind()), Some(ElementKind::Message(_))) {
                return None;
            }
            // A line from an element to itself keeps Astah's order of servers.
            if let Some((from, to)) = self.model.get(element).and_then(|e| edge_ends(e.kind()))
                && from != to
                && element_of(source) == Some(to)
                && element_of(target) == Some(from)
            {
                std::mem::swap(&mut source, &mut target);
                swapped = true;
            }
            Some(element)
        };
        let mut waypoints: Vec<Point> =
            self.r.points(p, "points").into_iter().map(|pt| pt.offset(shift.x, shift.y)).collect();
        // Astah lists the waypoints from its first server to the second.
        if swapped {
            waypoints.reverse();
        }
        let mut presentation = Presentation::edge(element, source, target, waypoints.clone());
        presentation.style.line_shape = match self.r.style(p, "line.shape").as_deref() {
            Some("line_right_angle") => LineShape::RightAngle,
            Some("curve") => LineShape::Curve,
            Some("curve_right_angle") => LineShape::CurveRightAngle,
            _ => LineShape::Line,
        };
        let curve = presentation.style.line_shape.is_curve();
        let bounds = |id: Id| self.model.view(diagram)?.get(id)?.bounds();
        // Lines end at the node's ellipse or diamond, as the scene draws them.
        let outline = |id: Id| {
            let p = self.model.view(diagram).and_then(|v| v.get(id));
            p.map_or(Outline::Rect, |p| scene::outline_of(&self.model, p))
        };
        if let (Some(source_rect), Some(target_rect)) = (bounds(source), bounds(target)) {
            let geometry = EdgeGeometry {
                source_rect,
                target_rect,
                outlines: [outline(source), outline(target)],
                waypoints,
                shift,
                swapped,
                curve,
            };
            if let Shape::Edge { source_anchor, target_anchor, labels, .. } =
                &mut presentation.shape
            {
                (*source_anchor, *target_anchor, *labels) = self.edge_layout(p, &geometry);
            }
        }
        let index = self.model.view(diagram).map_or(0, |v| v.presentations().len());
        // An edge that does not fit its ends is left out, like a presentation of a skipped
        // element.
        let id = presentation.id;
        if self.model.apply(Op::InsertPresentation { diagram, index, presentation }).is_err() {
            self.lost_presentations += 1;
            return None;
        }
        Some(id)
    }

    /// Where Astah's line aims inside its nodes, and where its labels sit. Labels are stored
    /// relative to the point they belong to, as the scene computes it: the end of the line for
    /// roles and multiplicities, the middle of the middle segment for the name.
    fn edge_layout(
        &self,
        p: ObjId,
        g: &EdgeGeometry,
    ) -> (Option<Point>, Option<Point>, EdgeLabels) {
        let r = self.r;
        // The center is where UmlCraft aims anyway; only other anchors are worth keeping.
        let anchor = |x, y| {
            Some(Point::new(r.num(p, x)? as f32, r.num(p, y)? as f32))
                .filter(|a| (a.x - 0.5).abs() > 1e-4 || (a.y - 0.5).abs() > 1e-4)
        };
        let (mut source_anchor, mut target_anchor) =
            (anchor("sourceEndX", "sourceEndY"), anchor("targetEndX", "targetEndY"));
        if g.swapped {
            std::mem::swap(&mut source_anchor, &mut target_anchor);
        }
        let inside = |rect: Rect, a: Option<Point>| match a {
            Some(a) => Point::new(rect.x + a.x * rect.width, rect.y + a.y * rect.height),
            None => rect.center(),
        };
        let (from, to) =
            (inside(g.source_rect, source_anchor), inside(g.target_rect, target_anchor));
        let [source_outline, target_outline] = g.outlines;
        let start = source_outline.border_point(
            g.source_rect,
            from,
            g.waypoints.first().copied().unwrap_or(to),
        );
        let end = target_outline.border_point(
            g.target_rect,
            to,
            g.waypoints.last().copied().unwrap_or(from),
        );
        let mut line = vec![start];
        line.extend(&g.waypoints);
        line.push(end);
        let (a, b) = route::middle_segment(&line, g.curve);
        let middle = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);

        // The top left corner of a label presentation, relative to `origin`.
        let place = |label: ObjId, origin: Point| {
            let at = r.obj(label, "location").and_then(|l| r.point(l))?;
            let moved = r.obj(label, "localMovement").and_then(|l| r.point(l)).unwrap_or_default();
            Some(Point::new(
                at.x + moved.x + g.shift.x - origin.x,
                at.y + moved.y + g.shift.y - origin.y,
            ))
        };
        let label = |field: &str, origin: Point| place(r.obj(p, field)?, origin);
        let (a_end, b_end) = if g.swapped { (end, start) } else { (start, end) };
        let (role_a, mult_a) =
            (label("roleAPresentation", a_end), label("multiplicityAPresentation", a_end));
        let (role_b, mult_b) =
            (label("roleBPresentation", b_end), label("multiplicityBPresentation", b_end));
        // «include» and «extend» are the stereotype label of the line.
        let keyword = r.list(p, "stereotypePresentations").first().and_then(|&k| place(k, middle));
        // A binding's substitutions (`<T->int>`) take the place of a name.
        let binding = r.class(p) == "TemplateBindingPresentation";
        let own_keyword = matches!(
            r.class(p),
            "IncludePresentation" | "ExtendPresentation" | "TemplateBindingPresentation"
        );
        let mut name =
            label(if binding { "boundInfoPresentation" } else { "namePresentation" }, middle);
        // Other lines show their stereotypes with the name; without a name they stand where
        // Astah keeps the stereotype label.
        let unnamed = r.obj(p, "model").is_none_or(|m| r.name(m).is_empty());
        if !own_keyword && unnamed && keyword.is_some() {
            name = keyword;
        }
        let mut labels =
            EdgeLabels { name, keyword: keyword.filter(|_| own_keyword), ..Default::default() };
        (
            labels.source_role,
            labels.source_multiplicity,
            labels.target_role,
            labels.target_multiplicity,
        ) = if g.swapped {
            (role_b, mult_b, role_a, mult_a)
        } else {
            (role_a, mult_a, role_b, mult_b)
        };
        (source_anchor, target_anchor, labels)
    }

    fn annotate(&mut self, comment: Id, annotated: Id) {
        let Some(e) = self.model.get(comment) else { return };
        let mut data = e.data().clone();
        if let ElementKind::Comment(props) = &mut data.kind
            && !props.annotated.contains(&annotated)
        {
            props.annotated.push(annotated);
            let _ = self.model.apply(Op::UpdateElement { id: comment, data });
        }
    }
}

/// An edge's ends and waypoints in UmlCraft coordinates, and whether its ends are in the
/// opposite order of Astah's.
struct EdgeGeometry {
    source_rect: Rect,
    target_rect: Rect,
    /// Where the line ends at each node, in UmlCraft's order.
    outlines: [Outline; 2],
    waypoints: Vec<Point>,
    shift: Point,
    swapped: bool,
    curve: bool,
}

struct NodeData {
    element: Option<Id>,
    bounds: Rect,
    style: Style,
    regions: Option<Regions>,
}

/// The elements an edge of this relationship runs from and to, as UmlCraft draws it.
fn edge_ends(kind: &ElementKind) -> Option<(Id, Id)> {
    match kind {
        ElementKind::Association(p) => Some((p.ends[0].participant, p.ends[1].participant)),
        ElementKind::Link(p) => Some((p.ends[0].instance, p.ends[1].instance)),
        ElementKind::Generalization(p) => Some((p.specific, p.general)),
        ElementKind::Realization(p) | ElementKind::Dependency(p) => Some((p.client, p.supplier)),
        ElementKind::Include(p) => Some((p.including, p.addition)),
        ElementKind::Extend(p) => Some((p.extension, p.extended)),
        ElementKind::Message(p) => Some((p.from, p.to)),
        ElementKind::ActivityEdge(p) => Some((p.source, p.target)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_profile_tags_are_kept() {
        for tag in ["jude.java.transient", "jude.c_sharp.override", "jude.c_plus.virtual"] {
            assert!(!is_internal_tag(tag), "{tag}");
        }
        for tag in ["jude.type", "jude.type_modifier", "jude.hyperlink", "hiddenDependency"] {
            assert!(is_internal_tag(tag), "{tag}");
        }
        assert_eq!(neutral_tag("jude.java.annotations"), "java.annotations");
        assert_eq!(neutral_tag("jude.c_sharp.override"), "csharp.override");
        assert_eq!(neutral_tag("jude.c_plus.virtual"), "cpp.virtual");
        assert_eq!(neutral_tag("author"), "author");
    }

    #[test]
    fn language_markers() {
        for marker in ["Java Class", "Java Attribute", "Java Method", "C# Class", "C++ Method"] {
            assert!(is_language_marker(marker), "{marker}");
        }
        for stereotype in ["abstract", "Java", "Java class", "Java Class Thing", "entity"] {
            assert!(!is_language_marker(stereotype), "{stereotype}");
        }
    }

    use crate::jser::{ClassData, ClassDesc, Object};

    /// Builds small Astah object graphs by hand.
    #[derive(Default)]
    struct Builder(Graph);

    impl Builder {
        fn class(&mut self, name: &str) -> crate::jser::ClassId {
            if let Some(i) = self.0.classes.iter().position(|c| c.name == name) {
                return crate::jser::ClassId(i);
            }
            self.0.classes.push(ClassDesc::new(name.to_owned(), 0, 0));
            crate::jser::ClassId(self.0.classes.len() - 1)
        }

        fn push(&mut self, object: Object) -> ObjId {
            self.0.objects.push(object);
            ObjId(self.0.objects.len() - 1)
        }

        fn string(&mut self, s: &str) -> Value {
            Value::Object(self.push(Object::String(s.to_owned())))
        }

        fn list(&mut self, items: &[ObjId]) -> Value {
            let class = self.class("[Ljava.lang.Object;");
            let items = items.iter().map(|&i| Value::Object(i)).collect();
            Value::Object(self.push(Object::Array { class, items }))
        }

        /// An instance of `class` without fields, to be filled by `set`.
        fn instance(&mut self, class: &str) -> ObjId {
            let class_id = self.class(class);
            let data = vec![ClassData { class: class.to_owned(), ..Default::default() }];
            self.push(Object::Instance { class: class_id, data })
        }

        fn set(&mut self, id: ObjId, field: &str, value: Value) {
            let Object::Instance { data, .. } = &mut self.0.objects[id.0] else { panic!() };
            data[0].fields.push((field.to_owned(), value));
        }

        /// Makes `owner` own `child` through an ownership object, as Astah does.
        fn own(&mut self, owner: ObjId, children: &[ObjId]) {
            let ownerships: Vec<ObjId> = children
                .iter()
                .map(|&child| {
                    let ownership = self.instance("JomtOwnership");
                    self.set(ownership, "ownedElement", Value::Object(child));
                    ownership
                })
                .collect();
            let list = self.list(&ownerships);
            self.set(owner, "ownedElementOwnership", list);
        }
    }

    #[test]
    fn a_package_that_owns_itself_is_imported_once() {
        let mut b = Builder::default();
        let model = b.instance("UModelImp");
        let name = b.string("Model");
        b.set(model, "name", name);
        let package = b.instance("UPackageImp");
        let name = b.string("P");
        b.set(package, "name", name);
        b.own(model, &[package]);
        b.own(package, &[package]);
        let imported = import(&b.0).unwrap();
        let m = &imported.model;
        let packages: Vec<_> = m.children(m.root()).to_vec();
        assert_eq!(packages.len(), 1);
        assert!(m.children(packages[0]).is_empty());
    }

    #[test]
    fn a_name_that_is_its_own_body_gives_no_text() {
        let mut b = Builder::default();
        let model = b.instance("UModelImp");
        let name = b.instance("UNameImp");
        b.set(name, "body", Value::Object(name));
        b.set(model, "name", Value::Object(name));
        let imported = import(&b.0).unwrap();
        assert_eq!(imported.model.element(imported.model.root()).unwrap().name(), "");
    }
}
