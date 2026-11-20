//! Machine resource reports and the offline plugin guest policy.

use serde::{Deserialize, Serialize};

/// Machine capacity. Placement accounts for both held machine slots and the
/// server's active leases. Available memory is sampled by the machine.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capacity {
    /// Logical CPUs.
    pub cpus: u32,
    /// Physical memory in bytes; 0 when unknown.
    pub memory_bytes: u64,
    /// Available physical memory in bytes at this heartbeat. Absent when the
    /// machine cannot provide a measurement. Admission waits for a sample.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_memory_bytes: Option<u64>,
    /// Live inventory and policy for the initialized offline plugin-VM backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_vm: Option<SmallVmCapacity>,
    /// Most provider and worker slots the machine can hold at once.
    pub max_sandboxes: u32,
    /// Slots currently held, including idle leases and pending removals.
    pub running_sandboxes: u32,
}

/// RAM in MiB reserved for each small plugin VM.
pub const SMALL_VM_MEMORY_MIB: u32 = 256;
/// Idle window in seconds when the small VM config omits one.
pub const SMALL_VM_IDLE_SECONDS: u32 = 15 * 60;

/// Longest operator-configured small VM idle window, in seconds.
pub const SMALL_VM_MAX_IDLE_SECONDS: u32 = 24 * 60 * 60;

/// Machine-local resource accounting for the offline plugin VM profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmallVmCapacity {
    /// Idle seconds before sleep, counting from the last completed call.
    pub idle_seconds: u32,
    /// Operator ceiling on reserved guest RAM, in bytes.
    pub memory_limit_bytes: u64,
    /// Effective resident limit after applying both the RAM and VMM ceilings.
    pub max_residents: u32,
    /// Guests with a RAM reservation, including boot and snapshot teardown.
    pub resident_count: u32,
    /// Total reserved guest RAM in bytes. Physical memory use varies by guest.
    pub resident_memory_bytes: u64,
}
