/** The message time formatter, made once: `toLocaleTimeString` builds a formatter on every call (about a millisecond each at 4x CPU slowdown, for every message drawn when a thread opens). */
export const clock = new Intl.DateTimeFormat("en-GB", { hour: "2-digit", minute: "2-digit", timeZone: "UTC" });
