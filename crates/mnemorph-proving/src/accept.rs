//! Where the harness reads a Dark Elf's report: each `@F`'s record is read and validated, and a
//! finding counts only when its record is [`Valid`] and its id is the record's fingerprint.

use mnemorph_wire::{DarkElfReport, Report};

use crate::finding::{FindingRecord, Rule, Valid, validate};
use crate::world::World;

/// A Dark Elf's report as the harness accepts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accepted {
    /// The report read as a Dark Elf's: no finding left in it; each one not proven rides as a
    /// hole — `invalid record (<rule>, …): <finding>` when its record broke the contract.
    pub report: DarkElfReport,
    valid: Vec<Valid>,
}

impl Accepted {
    /// The findings that count: proven on the wire, their records valid, one per proven `@F`.
    #[must_use]
    pub fn valid(&self) -> &[Valid] {
        &self.valid
    }
}

/// Reads `report` as a Dark Elf's against `world`, the landing's tree. Each `@F`'s record is read
/// at its `rec:` path and held to [`validate`]: an unreadable record demotes the finding to
/// `no record:`, a broken one to `invalid record (<rules>):`, and an id that is not the record's
/// fingerprint to `forged id:`, and a second finding of a root cause already counted to
/// `duplicate of #<id>:` — each an `@?`, never dropped.
#[must_use]
pub fn accept(report: Report, world: &(impl World + ?Sized)) -> Accepted {
    // The ids of the findings `dark_elf` reads a record for, in the order it reads them.
    let ids: Vec<(String, String)> = report
        .findings
        .iter()
        .filter(|f| f.has_dark_elf_parts())
        .filter_map(|f| Some((f.rec.clone()?, f.id.clone()?)))
        .collect();
    let mut next = ids.iter();
    let mut counted: Vec<String> = Vec::new();
    let mut seen: Vec<(String, Valid)> = Vec::new();
    let read = report.dark_elf(|rec| {
        // A second `@F` of one root cause is the first one again: it never counts twice.
        let id = next.next().filter(|(r, _)| r == rec).map(|(_, id)| id);
        if let Some(id) = id
            && counted.contains(id)
        {
            return Err(format!("duplicate of #{id}"));
        }
        let record = world
            .read(rec)
            .and_then(|bytes| FindingRecord::parse(rec, &bytes))
            .ok_or_else(|| "no record".to_owned())?;
        let valid = validate(&record, world).map_err(|rules| {
            let names: Vec<&str> = rules.iter().map(|r| Rule::name(*r)).collect();
            format!("invalid record ({})", names.join(", "))
        })?;
        let root = valid.root().clone();
        if let Some(id) = id
            && id == valid.fingerprint()
        {
            counted.push(id.clone());
        }
        seen.push((rec.to_owned(), valid));
        Ok(root)
    });
    let valid = read
        .proven()
        .iter()
        .filter_map(|p| {
            let f = p.finding();
            seen.iter()
                .find(|(rec, v)| {
                    f.rec.as_deref() == Some(rec.as_str())
                        && f.id.as_deref() == Some(v.fingerprint())
                })
                .map(|(_, v)| v.clone())
        })
        .collect();
    Accepted {
        report: read,
        valid,
    }
}
