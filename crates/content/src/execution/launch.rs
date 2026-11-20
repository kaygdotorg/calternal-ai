//! Launch reservation binding and exact reattachment. The stable reservation
//! UUID returned by the enrolled carrier is committed against the exact
//! attempt generation, machine and boot before StartEntry. A reconnect finds
//! the attempt only through that committed binding; nothing here relaunches.
use super::*;

/// Committed identity of one entry launch on an enrolled machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchBinding {
    /// Stable launch reservation (the carrier assignment, `origin` on the wire).
    pub reservation: Uuid,
    /// Enrolled machine selected at admission.
    pub machine: Uuid,
    /// Boot identity reported by that machine's authenticated session.
    pub boot: Uuid,
    /// Exact attempt ticket carried on every frame.
    pub ticket: Uuid,
    /// Contribution call identity whose output cursor Slice 1 tracks.
    pub call: String,
}

/// What a reconnect may do for one committed reservation.
#[derive(Debug, PartialEq)]
pub enum Reattach {
    /// Same boot and live authority: send `ResumeEntry` with the binding's
    /// reservation, then deliver any outstanding controls.
    Resume {
        /// Exact attempt to resume.
        attempt: Attempt,
        /// Committed launch identity.
        binding: LaunchBinding,
        /// Retained pin, for renewal of the runtime grant.
        pin: Box<PinnedExecution>,
    },
    /// The attempt is retired (or just lost authority): attach only to
    /// deliver its exact Stop, settle committed output and prove teardown.
    /// Admits no new effects, renewals or output publication.
    CleanupOnly {
        /// Exact retired attempt.
        attempt: Attempt,
        /// Committed launch identity.
        binding: LaunchBinding,
    },
    /// A new machine boot: the runtime is gone. Interruption is committed
    /// and the attempt waits for cleanup; no replacement is started.
    Failed {
        /// Exact attempt now retired as interrupted.
        attempt: Attempt,
    },
}

/// Result of verifying `EntryAttached` against the committed binding.
#[derive(Debug, Eq, PartialEq)]
pub enum Attached {
    /// The live runner is the original launch; relay may continue.
    Live(Attempt),
    /// The original runner is attached for cleanup only.
    CleanupOnly(Attempt),
}

/// Commit the launch binding before StartEntry. Requires the exact live
/// attempt, the admitted machine and a passing live check on the retained
/// pin. Repeating the identical binding is a no-op; a different one for the
/// same attempt, or a reservation already bound elsewhere, conflicts.
/// Cost: one indexed attempt read, one binding read and one insert.
pub fn bind_launch(
    tx: &Transaction<'_>,
    attempt: &Attempt,
    binding: &LaunchBinding,
    check: impl FnOnce(&PinnedExecution) -> Result<bool>,
) -> Result<()> {
    let pin = live_pin(tx, attempt)?.ok_or_else(unavailable)?;
    if pin.machine != binding.machine || !live(&pin, check)? {
        return Err(unavailable());
    }
    if let Some((bound_attempt, bound)) = lookup(tx, &binding.reservation)? {
        return if &bound_attempt == attempt && &bound == binding {
            Ok(())
        } else {
            Err(Error::conflict("Launch reservation is already bound"))
        };
    }
    tx.prepare_cached(
        "INSERT INTO execution_launches(execution,generation,reservation,machine,boot,ticket,call)
         VALUES(?1,?2,?3,?4,?5,?6,?7)",
    )
    .map_err(internal)?
    .execute(params![
        attempt.execution.0,
        attempt.generation,
        binding.reservation.to_string(),
        binding.machine.to_string(),
        binding.boot.to_string(),
        binding.ticket.to_string(),
        binding.call
    ])
    .map_err(|_| Error::conflict("Launch reservation is already bound"))?;
    Ok(())
}

fn lookup(tx: &Transaction<'_>, reservation: &Uuid) -> Result<Option<(Attempt, LaunchBinding)>> {
    let row = tx
        .prepare_cached(
            "SELECT execution,generation,machine,boot,ticket,call FROM execution_launches
             WHERE reservation=?1",
        )
        .map_err(internal)?
        .query_row([reservation.to_string()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .optional()
        .map_err(internal)?;
    let Some((execution, generation, machine, boot, ticket, call)) = row else {
        return Ok(None);
    };
    let uuid = |value: String| Uuid::parse_str(&value).map_err(internal);
    Ok(Some((
        Attempt {
            execution: ExecutionRef(execution),
            generation,
        },
        LaunchBinding {
            reservation: *reservation,
            machine: uuid(machine)?,
            boot: uuid(boot)?,
            ticket: uuid(ticket)?,
            call,
        },
    )))
}

/// Decide reattachment for one reservation offered on a fresh authenticated
/// session of `machine` with `boot`. Unknown reservations and another
/// machine's session return `None` with no effect, so a forged or foreign
/// offer cannot touch a live attempt. A new boot commits interruption. A
/// failed live check commits interruption with an exact Stop and returns the
/// cleanup-only path. Cost: one indexed binding read, one attempt read and at
/// most one retirement.
pub fn reattach(
    tx: &Transaction<'_>,
    reservation: &Uuid,
    machine: &Uuid,
    boot: &Uuid,
    check: impl FnOnce(&PinnedExecution) -> Result<bool>,
) -> Result<Option<Reattach>> {
    let Some((attempt, binding)) = lookup(tx, reservation)? else {
        return Ok(None);
    };
    if &binding.machine != machine {
        return Ok(None);
    }
    let Some(pin) = live_pin(tx, &attempt)? else {
        return Ok(Some(Reattach::CleanupOnly { attempt, binding }));
    };
    let fence = format!("reattach/{reservation}");
    if &binding.boot != boot {
        fence::end_attempt(tx, &attempt.execution, "interrupted", &fence)?;
        return Ok(Some(Reattach::Failed { attempt }));
    }
    if !live(&pin, check)? {
        fence::end_attempt(tx, &attempt.execution, "interrupted", &fence)?;
        return Ok(Some(Reattach::CleanupOnly { attempt, binding }));
    }
    Ok(Some(Reattach::Resume {
        attempt,
        binding,
        pin: Box::new(pin),
    }))
}

/// Verify the machine's `EntryAttached { origin, buffered_through }` after
/// `ResumeEntry { origin: reservation, after }`. The origin must be the
/// committed reservation on the same machine boot and attempt generation, and
/// the machine cannot hold fewer frames than the host already committed.
/// Any mismatch is refused with no state change. Cost: two indexed reads.
pub fn verify_attached(
    tx: &Transaction<'_>,
    reservation: &Uuid,
    boot: &Uuid,
    origin: &Uuid,
    buffered_through: u64,
    after: u64,
) -> Result<Attached> {
    let (attempt, binding) = lookup(tx, reservation)?.ok_or_else(unavailable)?;
    if origin != reservation || &binding.boot != boot || buffered_through < after {
        return Err(unavailable());
    }
    let ended = tx
        .prepare_cached(
            "SELECT end_reason IS NOT NULL FROM execution_attempts
             WHERE execution=?1 AND generation=?2",
        )
        .map_err(internal)?
        .query_row(params![attempt.execution.0, attempt.generation], |r| {
            r.get::<_, bool>(0)
        })
        .optional()
        .map_err(internal)?
        .ok_or_else(unavailable)?;
    Ok(if ended {
        Attached::CleanupOnly(attempt)
    } else {
        Attached::Live(attempt)
    })
}
