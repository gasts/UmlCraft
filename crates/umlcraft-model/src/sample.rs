//! A small example project with one diagram of each supported kind.
//! Used by tests, the format round trip and as the app's start document.

use crate::diagram::DiagramKind;
use crate::document::Document;
use crate::element::{
    Aggregation, ElementData, ElementKind, LifelineProps, MessageSort, Multiplicity, TypeRef,
    Visibility,
};
use crate::geometry::Rect;
use crate::id::Id;
use crate::model::Model;

/// Ids of the interesting elements in [`order_system`].
#[derive(Clone, Copy, Debug)]
pub struct OrderSystem {
    pub domain: Id,
    pub customer: Id,
    pub order: Id,
    pub order_customer: Id,
    pub premium: Id,
    pub premium_generalization: Id,
    pub class_diagram: Id,
    pub use_case_diagram: Id,
    pub sequence_diagram: Id,
    pub customer_lifeline: Id,
}

pub fn order_system() -> (Document, OrderSystem) {
    let mut doc = Document::new(Model::new("Order System"));
    let root = doc.model().root();
    let ids = doc
        .edit("Create sample", |tx| {
            let domain = tx.create(root, ElementData::new("domain", ElementKind::Package))?;

            let customer = tx.create(domain, ElementData::new("Customer", ElementKind::class()))?;
            tx.create(
                customer,
                ElementData::new(
                    "name",
                    ElementKind::attribute(Some(TypeRef::Text("String".into()))),
                )
                .with_visibility(Visibility::Private),
            )?;
            let order = tx.create(domain, ElementData::new("Order", ElementKind::class()))?;
            tx.create(
                order,
                ElementData::new(
                    "total",
                    ElementKind::attribute(Some(TypeRef::Text("Money".into()))),
                )
                .with_visibility(Visibility::Private),
            )?;
            tx.create(order, ElementData::new("submit", ElementKind::operation(None)))?;
            let premium =
                tx.create(domain, ElementData::new("PremiumCustomer", ElementKind::class()))?;

            let mut association = ElementKind::association(order, customer);
            if let ElementKind::Association(p) = &mut association {
                p.ends[0].multiplicity = Some(Multiplicity::MANY);
                p.ends[1].multiplicity = Some(Multiplicity::ONE);
                p.ends[1].role = "customer".into();
                p.ends[0].aggregation = Aggregation::None;
            }
            let order_customer = tx.create(domain, ElementData::new("", association))?;
            let premium_generalization = tx.create(
                domain,
                ElementData::new("", ElementKind::generalization(premium, customer)),
            )?;

            let class_diagram = tx.create(
                domain,
                ElementData::new("Domain", ElementKind::diagram(DiagramKind::Class)),
            )?;
            let n_customer =
                tx.add_node(class_diagram, Some(customer), Rect::new(80.0, 80.0, 160.0, 90.0))?;
            let n_order =
                tx.add_node(class_diagram, Some(order), Rect::new(380.0, 80.0, 160.0, 100.0))?;
            let n_premium =
                tx.add_node(class_diagram, Some(premium), Rect::new(80.0, 260.0, 160.0, 60.0))?;
            tx.add_edge(class_diagram, Some(order_customer), n_order, n_customer, vec![])?;
            tx.add_edge(
                class_diagram,
                Some(premium_generalization),
                n_premium,
                n_customer,
                vec![],
            )?;

            let actor = tx.create(root, ElementData::new("Clerk", ElementKind::actor()))?;
            let place_order =
                tx.create(root, ElementData::new("Place order", ElementKind::use_case()))?;
            let actor_link = tx
                .create(root, ElementData::new("", ElementKind::association(actor, place_order)))?;
            let use_case_diagram = tx.create(
                root,
                ElementData::new("Use cases", ElementKind::diagram(DiagramKind::UseCase)),
            )?;
            let n_actor =
                tx.add_node(use_case_diagram, Some(actor), Rect::new(60.0, 100.0, 40.0, 80.0))?;
            let n_uc = tx.add_node(
                use_case_diagram,
                Some(place_order),
                Rect::new(220.0, 110.0, 140.0, 60.0),
            )?;
            tx.add_edge(use_case_diagram, Some(actor_link), n_actor, n_uc, vec![])?;

            let interaction =
                tx.create(place_order, ElementData::new("Place order", ElementKind::Interaction))?;
            let customer_lifeline = tx.create(
                interaction,
                ElementData::new(
                    "c",
                    ElementKind::Lifeline(LifelineProps { represents: Some(customer) }),
                ),
            )?;
            let order_lifeline = tx.create(
                interaction,
                ElementData::new(
                    "o",
                    ElementKind::Lifeline(LifelineProps { represents: Some(order) }),
                ),
            )?;
            tx.create(
                interaction,
                ElementData::new(
                    "submit",
                    ElementKind::message(
                        MessageSort::Synchronous,
                        customer_lifeline,
                        order_lifeline,
                    ),
                ),
            )?;
            let sequence_diagram = tx.create(
                interaction,
                ElementData::new("Place order", ElementKind::diagram(DiagramKind::Sequence)),
            )?;
            tx.add_node(
                sequence_diagram,
                Some(customer_lifeline),
                Rect::new(60.0, 40.0, 100.0, 300.0),
            )?;
            tx.add_node(
                sequence_diagram,
                Some(order_lifeline),
                Rect::new(260.0, 40.0, 100.0, 300.0),
            )?;

            Ok(OrderSystem {
                domain,
                customer,
                order,
                order_customer,
                premium,
                premium_generalization,
                class_diagram,
                use_case_diagram,
                sequence_diagram,
                customer_lifeline,
            })
        })
        .expect("sample model is valid");
    doc.clear_history();
    (doc, ids)
}
