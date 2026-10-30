// SPDX-License-Identifier: LGPL-2.1-or-later
//! Taking and restoring [`Snapshot`]s of the executive, and the loads'
//! record that fingerprints them.

use tracing::error;

use super::{ExecState, Fdm, Models};
use crate::script::{Event, Script, Set};
use crate::snapshot::{
    Checker, Fingerprint, HashingResolver, ShapeHasher, Snapshot, SnapshotError, Visit, Visitor,
    Writer, visit_fields, visit_present,
};
use crate::{Error, Resolver};

// `requests` is drained by the write that queues a request, before the
// write returns, so it is empty whenever a snapshot can be taken.
visit_fields! {
    ExecState {
        sim_time, dt, saved_dt, frame, holding, terminate, hold_down, trim_status, trim_completed,
        random_seed, rng, debug_level, wall_clock,
    }
    skip { requests }
}

visit_fields! {
    Models {
        exec, inertial, propagate, atmosphere, winds, fcs, mass_balance, auxiliary, propulsion,
        aerodynamics, ground_reactions, external_reactions, buoyant_forces, aircraft, accelerations,
        input, output, ic,
    }
}

visit_fields! {
    Set { node, value, original, new_value, span, tc, transiting }
    skip { name, function, kind, action }
}

impl Visit for Event {
    fn visit<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        let Self {
            persistent,
            continuous,
            condition,
            delay,
            notify,
            notify_properties,
            sets,
            triggered,
            notified,
            start_time,
            name: _,
            description: _,
        } = self;
        for x in [persistent, continuous, notify, triggered, notified] {
            x.visit(v)?;
        }
        delay.visit(v)?;
        start_time.visit(v)?;
        condition.visit(v)?;
        v.count(notify_properties.len(), "notified properties")?;
        for (parameter, _caption) in notify_properties.iter_mut() {
            parameter.visit(v)?;
        }
        sets.visit(v)
    }
}

visit_fields! {
    Script { events, events_reset }
    skip { name, start_time, end_time, local_properties }
}

impl Fdm {
    /// Runs `load` with a resolver that hashes every document it resolves
    /// into the fingerprint, then records the tree the load left.
    pub(super) fn hashed_load<T>(
        &mut self,
        resolver: &dyn Resolver,
        load: impl FnOnce(&mut Self, &dyn Resolver) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let hashing = HashingResolver::new(resolver);
        let result = load(self, &hashing);
        let digest = hashing.finish();
        let nodes = self.env.tree.node_count();
        self.snapshot_record.loaded(digest, nodes);
        result
    }

    /// Records a load that read no document, such as a parsed element.
    pub(super) fn loaded_without_documents(&mut self) {
        let nodes = self.env.tree.node_count();
        self.snapshot_record.structure_changed(nodes);
    }

    /// The fingerprint a snapshot of this executive carries.
    pub fn fingerprint(&mut self) -> Result<Fingerprint, Error> {
        Ok(self.current_fingerprint()?)
    }

    fn current_fingerprint(&mut self) -> Result<Fingerprint, SnapshotError> {
        if let Some(fingerprint) = self.snapshot_record.cached() {
            return Ok(fingerprint);
        }
        let load_nodes = self.snapshot_record.load_nodes();
        let mut shape = ShapeHasher::default();
        self.visit_state(&mut shape)?;
        let fingerprint = Fingerprint {
            documents: self.snapshot_record.documents(),
            tree: self.env.tree.shape_digest(load_nodes),
            models: shape.finish(),
            load_nodes,
        };
        self.snapshot_record.cache(fingerprint);
        Ok(fingerprint)
    }

    /// Saves the dynamic state into `snapshot`, reusing its buffer. Call it
    /// between [`run`](Self::run)s; a trim, a reset or a linearization a
    /// property write requests has finished when the write returns.
    pub fn snapshot_into(&mut self, snapshot: &mut Snapshot) -> Result<(), Error> {
        let fingerprint = self.current_fingerprint()?;
        let buffer = snapshot.buffer_mut();
        buffer.clear();
        let mut w = Writer::new(buffer);
        fingerprint.write_header(&mut w);
        self.env
            .tree
            .write_runtime_nodes(&mut w, fingerprint.load_nodes);
        self.env.tree.visit_values(&mut w)?;
        self.visit_state(&mut w)?;
        Ok(())
    }

    /// Restores the state `snapshot` saved. The snapshot must be of this
    /// model loaded the same way: [`SnapshotError::FingerprintMismatch`]
    /// otherwise. Property nodes created after the snapshot are dropped,
    /// so their ids become invalid; nodes it holds that the executive
    /// lacks are made. The ground callback stays as installed.
    ///
    /// The whole snapshot is decoded and checked before anything is
    /// written, so every error, a corrupted or truncated body included,
    /// leaves the executive unchanged.
    pub fn restore(&mut self, snapshot: &Snapshot) -> Result<(), Error> {
        let bytes = snapshot.as_bytes();
        let theirs = Fingerprint::read_header(bytes)?;
        let ours = self.current_fingerprint()?;
        if let Some(part) = ours.first_difference(&theirs) {
            return Err(SnapshotError::FingerprintMismatch { part }.into());
        }
        self.check_body(bytes, theirs.load_nodes)?;
        self.read_body(bytes, theirs.load_nodes).map_err(|e| {
            // The check decodes as the read does, so this is a bug in a
            // `Visit` impl, and the state is partly restored.
            error!(error = %e, "a snapshot that passed its check failed to restore");
            e.into()
        })
    }

    /// Decodes the body as [`read_body`](Self::read_body) does, writing
    /// nothing and allocating nothing.
    fn check_body(&mut self, bytes: &[u8], load_nodes: u32) -> Result<(), SnapshotError> {
        let mut c = Checker::new(Fingerprint::body(bytes));
        let runtime = self.env.tree.check_runtime_nodes(c.reader(), load_nodes)?;
        self.env.tree.check_values(&mut c, runtime)?;
        self.visit_state(&mut c)?;
        c.finish()
    }

    fn read_body(&mut self, bytes: &[u8], load_nodes: u32) -> Result<(), SnapshotError> {
        let mut r = Fingerprint::body(bytes);
        self.env.tree.read_runtime_nodes(&mut r, load_nodes)?;
        self.env.tree.visit_values(&mut r)?;
        self.visit_state(&mut r)?;
        r.finish()
    }

    /// The state past the property tree, in one order for every
    /// direction.
    fn visit_state<V: Visitor>(&mut self, v: &mut V) -> Result<(), SnapshotError> {
        self.models.visit(v)?;
        self.env.functions.visit(v)?;
        visit_present(&mut self.script, v)?;
        self.trims_run.visit(v)
    }
}
