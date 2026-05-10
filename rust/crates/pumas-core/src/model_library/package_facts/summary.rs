use crate::models::{ResolvedModelPackageFacts, ResolvedModelPackageFactsSummary};

pub(crate) fn package_facts_summary(
    facts: &ResolvedModelPackageFacts,
) -> ResolvedModelPackageFactsSummary {
    ResolvedModelPackageFactsSummary::from(facts)
}
