/**
 * Mail's fixture data: conversations, drafts, and the collections of the sidebar. Invented people and text; nothing here is real or sent.
 * A mail item (`kind: "mail"`) is one conversation: its `title` is the sender of the latest message, its `sub` the subject. The messages live in `THREADS`, by item id; an id with no entry draws as a single message made from the row.
 * "Today" is Friday 20 November 2026 (the mockup's fixed clock).
 */
import { def, ITEMS, mk, pick, type Item } from "../../data/collections";

export interface Att { name: string; size: string; type: "pdf" | "image" | "calendar" | "sheet" | "zip" | "doc" }
export interface Msg {
  /** Display name. */ from: string; /** Who it went to, as the page says it ("me", "me, Ivo Novak"). */ to: string; cc?: string;
  /** Full date for the header line. */ at: string; /** Short time for the collapsed row. */ short: string;
  paras: string[]; /** The text of the message it answers, folded by default: a "wrote" line and the lines. */ quoted?: { head: string; lines: string[] }; atts?: Att[]; unread?: boolean;
  /** Sent by me: the sender line reads "Me". */ mine?: boolean; /** A mailing list or bulk sender: no reply-all. */ bulk?: boolean;
}
export interface Thread { subject: string; account: "Personal" | "Studio"; labels: string[]; flagged?: boolean; msgs: Msg[]; /** Items elsewhere this mail is about (a calendar event, a task, a chat): the right panel's Related. */ related?: string[] }

const A = (name: string, size: string, type: Att["type"]): Att => ({ name, size, type });
const M = (m: Msg): Msg => m;

export const THREADS: Record<string, Thread> = {
  "m-0": { subject: "Your booking is confirmed: Lisbon, Friday 27 November", account: "Personal", labels: ["Travel", "Receipts"], flagged: true, related: ["e-1", "t-4"], msgs: [
    M({ from: "Airline", to: "me", at: "Friday 20 November, 09:41", short: "09:41", unread: true, bulk: true, paras: ["Hello Ada,", "Your flight is confirmed. Check-in opens 24 hours before departure, and your boarding passes will be in the app and attached below.", "Departing Friday 27 November, 07:10. Arriving 09:45 (local time). One cabin bag and one personal item are included for each traveller."], atts: [A("Booking confirmation AB4K7Q.pdf", "212 KB", "pdf"), A("Flight 27 Nov.ics", "1 KB", "calendar")] }),
  ] },
  "m-1": { subject: "Check-in instructions", account: "Personal", labels: ["Travel"], related: ["e-1"], msgs: [
    M({ from: "Marta Silva", to: "me", at: "Friday 20 November, 08:15", short: "08:15", unread: true, paras: ["Dear Ada,", "We look forward to welcoming you. Check-in is from 15:00; if you arrive earlier we will keep your bags at the desk. The door code for the street entrance will be sent on the morning of arrival.", "The lift is small, so the steps to the second floor are sometimes quicker with a suitcase."] }),
  ] },
  "m-2": { subject: "Photos from the walk", account: "Personal", labels: [], related: ["c-hana"], msgs: [
    M({ from: "Hana Sato", to: "me", at: "Thursday 19 November, 18:20", short: "Thu", paras: ["The light was perfect on the hill. I put the good ones in the shared album; the blurry ones I kept for myself.", "Are you still free on Saturday morning? Eva wants to try the new place near the river."], atts: [A("IMG_4127.jpg", "2.4 MB", "image"), A("IMG_4131.jpg", "2.1 MB", "image"), A("IMG_4140.jpg", "2.7 MB", "image")] }),
    M({ from: "Ada Lindqvist", mine: true, to: "Hana Sato", at: "Thursday 19 November, 19:02", short: "Thu", paras: ["These are lovely, thank you. Saturday morning works; shall we say ten?"], quoted: { head: "On Thursday 19 November, 18:20, Hana Sato wrote:", lines: ["The light was perfect on the hill. I put the good ones in the shared album; the blurry ones I kept for myself.", "Are you still free on Saturday morning? Eva wants to try the new place near the river."] } }),
    M({ from: "Hana Sato", to: "me", at: "Yesterday, 21:48", short: "Yesterday", unread: false, paras: ["Ten is perfect. I will bring the thermos.", "Eva says she will meet us there."], quoted: { head: "On Thursday 19 November, 19:02, Ada Lindqvist wrote:", lines: ["These are lovely, thank you. Saturday morning works; shall we say ten?"] } }),
  ] },
  "m-3": { subject: "About the lease renewal", account: "Personal", labels: ["Home"], flagged: true, related: ["ev-6", "c-flat"], msgs: [
    M({ from: "Femi Hansen", to: "me", at: "Monday 16 November, 10:05", short: "Mon", unread: true, paras: ["Good morning Ada,", "The current lease ends on 31 December. I would like to renew for another twelve months. The only change is a small adjustment of the rent, about three per cent, which I explain in the attached letter.", "If you agree, I can send the signed copy by the end of the month. If you would rather talk it through, call me on my mobile in the evening."], atts: [A("Lease renewal letter.pdf", "186 KB", "pdf"), A("Rent schedule 2027.xlsx", "34 KB", "sheet")] }),
  ] },
  "m-4": { subject: "Your statement is ready", account: "Personal", labels: ["Receipts"], msgs: [
    M({ from: "Bank", to: "me", at: "Sunday 15 November, 06:00", short: "Sun", bulk: true, paras: ["Your October statement is now available.", "For your security we never include account numbers or balances in an email. Sign in to the app to read it."] }),
  ] },
  "m-5": { subject: "Receipt for 14 Nov", account: "Personal", labels: ["Receipts"], msgs: [
    M({ from: "Café Central", to: "me", at: "Saturday 14 November, 12:31", short: "Sat", bulk: true, paras: ["Thank you for visiting Café Central.", "Two coffees and a pastry, €7.40. VAT 13 %."], atts: [A("Receipt 0041.pdf", "18 KB", "pdf")] }),
  ] },
  "mx-0": { subject: "Build is green again", account: "Studio", labels: ["Work"], related: ["c-ivo"], msgs: [
    M({ from: "Ivo Novak", to: "me, Chloe Rossi", at: "Friday 20 November, 09:02", short: "09:02", paras: ["The flaky test was a clock problem: it assumed the machine was in Lisbon. Fixed and pushed.", "Can you take another look at the sidebar spacing before lunch?"], quoted: undefined }),
    M({ from: "Chloe Rossi", to: "Ivo Novak, me", at: "Friday 20 November, 09:40", short: "09:40", paras: ["Thank you, Ivo. I will look at the spacing after the stand-up."] }),
    M({ from: "Ivo Novak", to: "me, Chloe Rossi", at: "Friday 20 November, 10:12", short: "10:12", unread: true, paras: ["No hurry. One more thing: the release note draft is in the shared doc, comments welcome."], quoted: { head: "On Friday 20 November, 09:40, Chloe Rossi wrote:", lines: ["Thank you, Ivo. I will look at the spacing after the stand-up.", "", "On Friday 20 November, 09:02, Ivo Novak wrote:", "> The flaky test was a clock problem: it assumed the machine was in Lisbon. Fixed and pushed."] } }),
  ] },
  "mx-1": { subject: "Documents for the tax return", account: "Personal", labels: ["Receipts"], related: ["c-paper"], msgs: [
    M({ from: "Anna Okafor", to: "me", at: "Wednesday 18 November, 15:20", short: "Wed", paras: ["Hello Ada,", "I have the draft return ready. Please check the figures on page two and send me the missing receipts; the list is in the second attachment.", "Kind regards, Anna"], atts: [A("Return 2026 draft.pdf", "540 KB", "pdf"), A("Missing receipts.xlsx", "22 KB", "sheet"), A("Receipts Oct.zip", "6.8 MB", "zip")] }),
  ] },
  "mx-2": { subject: "The Slow Traveller, issue 142: Alentejo in the rain", account: "Personal", labels: [], msgs: [
    M({ from: "The Slow Traveller", to: "me", at: "Tuesday 17 November, 07:30", short: "Tue", bulk: true, paras: ["This week: cork forests after the first autumn rain, a ferry that only runs on Thursdays, and a baker in Évora who will not tell anyone his recipe.", "Read it in the browser, or reply to say hello."] }),
  ] },
  "mx-3": { subject: "Invoice 2026-114", account: "Studio", labels: ["Work", "Receipts"], msgs: [
    M({ from: "Dev Patel", to: "me", at: "Tuesday 17 November, 11:14", short: "Tue", paras: ["Invoice for the November sessions is attached. Payment terms are the usual thirty days."], atts: [A("Invoice 2026-114.pdf", "96 KB", "pdf")] }),
  ] },
  "mx-4": { subject: "Re: Planning review: agenda and pre-reading", account: "Studio", labels: ["Work"], flagged: true, related: ["e-2", "ev-4"], msgs: [
    M({ from: "Chloe Rossi", to: "me, Ivo Novak, Dev Patel", at: "Monday 9 November, 10:00", short: "9 Nov", paras: ["Hello all, the quarterly planning review is on Monday 23 November at 14:00. Proposed agenda below; please add what is missing.", "1. What shipped, 2. What slipped and why, 3. The three bets for next quarter, 4. Hiring."] }),
    M({ from: "Ivo Novak", to: "Chloe Rossi, me, Dev Patel", at: "Monday 9 November, 10:41", short: "9 Nov", paras: ["Can we add a short item on the build times? It cost us about two days last month."], quoted: { head: "On Monday 9 November, 10:00, Chloe Rossi wrote:", lines: ["Hello all, the quarterly planning review is on Monday 23 November at 14:00."] } }),
    M({ from: "Dev Patel", to: "Chloe Rossi, me, Ivo Novak", at: "Monday 9 November, 11:15", short: "9 Nov", paras: ["Added. I will also bring the cost figures for the new test machines."] }),
    M({ from: "Ada Lindqvist", mine: true, to: "Chloe Rossi, Ivo Novak, Dev Patel", at: "Tuesday 10 November, 08:55", short: "10 Nov", paras: ["Thanks, all. I would like twenty minutes for the three bets, not ten; they are the part that needs a discussion."] }),
    M({ from: "Chloe Rossi", to: "me, Ivo Novak, Dev Patel", at: "Tuesday 10 November, 09:20", short: "10 Nov", paras: ["Fair. I will shorten the shipping review to match."] }),
    M({ from: "Ivo Novak", to: "Chloe Rossi, me, Dev Patel", at: "Wednesday 11 November, 14:03", short: "11 Nov", paras: ["Pre-reading attached. It is long, I know. Section four is the one to read if you read only one."], atts: [A("Q4 pre-reading.pdf", "1.4 MB", "pdf"), A("Build times, last 90 days.png", "310 KB", "image")] }),
    M({ from: "Dev Patel", to: "Chloe Rossi, me, Ivo Novak", at: "Thursday 12 November, 09:10", short: "12 Nov", paras: ["Read it. Two small corrections in the figures; I have commented in the document."] }),
    M({ from: "Chloe Rossi", to: "me, Ivo Novak, Dev Patel", at: "Wednesday 18 November, 16:30", short: "18 Nov", paras: ["Reminder: Monday, 14:00, the long room. Please bring questions, not slides.", "A last request: could everybody who is travelling next week tell me by Friday, so I can set up a call link?"], quoted: { head: "On Thursday 12 November, 09:10, Dev Patel wrote:", lines: ["Read it. Two small corrections in the figures; I have commented in the document."] } }),
    M({ from: "Ada Lindqvist", mine: true, to: "Chloe Rossi, Ivo Novak, Dev Patel", at: "Wednesday 18 November, 17:02", short: "18 Nov", paras: ["I am in Lisbon on Friday the 27th, so not travelling next week. See you in the long room."] }),
    M({ from: "Ivo Novak", to: "me, Chloe Rossi, Dev Patel", at: "Friday 20 November, 08:41", short: "08:41", unread: true, paras: ["Last thing from me: I moved the build-times item to the top, since it explains the slip. A note about the very long thread of replies, in the hope that this window shows how the page copes with a conversation that goes on for a while, with a long line that has no natural break so that we can see whether it wraps or runs away: aVeryLongWordWithoutAnyBreaksInItThatCouldBeAnAddressOrATrackingLinkOrAnythingElseThatIsInconvenient."], quoted: { head: "On Wednesday 18 November, 17:02, Ada Lindqvist wrote:", lines: ["I am in Lisbon on Friday the 27th, so not travelling next week. See you in the long room."] } }),
  ] },
  "mx-5": { subject: "Appointment reminder: Saturday 21 November, 09:30", account: "Personal", labels: ["Home"], related: ["e-0"], msgs: [
    M({ from: "Dr. Sofia Ferreira", to: "me", at: "Thursday 19 November, 12:00", short: "Thu", bulk: true, paras: ["This is a reminder of your appointment on Saturday 21 November at 09:30 at Clínica do Chiado.", "Please arrive ten minutes early. If you cannot come, reply to this message at least a day before."] }),
  ] },
  "mx-6": { subject: "Boiler service in March", account: "Personal", labels: ["Home"], msgs: [
    M({ from: "Femi Hansen", to: "me", at: "Tuesday 3 November, 14:30", short: "3 Nov", paras: ["The boiler man will come on a Tuesday in the second week of March. I will confirm the exact day the week before."] }),
  ] },
  "mx-7": { subject: "Spare key", account: "Personal", labels: ["Home"], msgs: [
    M({ from: "Femi Hansen", to: "me", at: "Monday 19 October, 09:12", short: "19 Oct", paras: ["Lena has the spare key for the flat, as you asked. The office keeps the other one."] }),
  ] },
  "mx-8": { subject: "Re: Rent receipt for September", account: "Personal", labels: ["Home", "Receipts"], msgs: [
    M({ from: "Femi Hansen", to: "me", at: "Friday 2 October, 16:48", short: "2 Oct", paras: ["Attached, as asked."], atts: [A("Receipt September.pdf", "41 KB", "pdf")] }),
  ] },
};

/** A draft: the fields the compose window starts with. `replyTo` is the thread it answers (its quote is below the cursor). */
export interface Draft { from: "Personal" | "Studio"; to: string[]; cc: string[]; subject: string; body: string; atts: Att[]; replyTo?: string; saved: string }
export const DRAFTS: Record<string, Draft> = {
  "draft-new": { from: "Personal", to: [], cc: [], subject: "", body: "", atts: [], saved: "" },
  "dr-0": { from: "Personal", to: ["Femi Hansen"], cc: [], subject: "Re: About the lease renewal", replyTo: "m-3", saved: "Saved 17:48", atts: [], body: "Dear Femi,\n\nThank you for the letter. Three per cent is fine in principle, but could we keep the rent as it is until March, when the boiler is serviced? I would also like to ask about the parking space we discussed in the summer.\n\nI can call you on Sunday evening if that suits you." },
  "dr-1": { from: "Studio", to: ["Chloe Rossi"], cc: ["Ivo Novak"], subject: "Release note draft: a few comments", saved: "Saved Wednesday", atts: [A("Release notes v3.md", "6 KB", "doc")], body: "Hello Chloe,\n\nI read the draft and have three small comments, mostly about the order. " },
};

// The conversations' items. Existing shell ids (m-0 to m-5) keep their rows; new ones are registered here.
const mx = (id: string, who: string, subject: string, when: string, x: Partial<Item> = {}): Item => ITEMS[id] ?? mk("mail", id, who, subject, { when, ...x });
mx("mx-0", "Ivo Novak", "Build is green again", "10:12", { unread: true }); mx("mx-1", "Anna Okafor", "Documents for the tax return", "Wed"); mx("mx-2", "The Slow Traveller", "Issue 142: Alentejo in the rain", "Tue"); mx("mx-3", "Dev Patel", "Invoice 2026-114", "Tue");
mx("mx-4", "Ivo Novak", "Re: Planning review: agenda and pre-reading", "08:41", { unread: true, n: 10 }); mx("mx-5", "Dr. Sofia Ferreira", "Appointment reminder: Saturday 09:30", "Thu"); mx("mx-6", "Femi Hansen", "Boiler service in March", "3 Nov"); mx("mx-7", "Femi Hansen", "Spare key", "19 Oct"); mx("mx-8", "Femi Hansen", "Re: Rent receipt for September", "2 Oct");
mk("mail", "dr-0", "Femi Hansen", "Re: About the lease renewal", { when: "17:48" }); mk("mail", "dr-1", "Chloe Rossi", "Release note draft: a few comments", { when: "Wed" });
mk("mail", "draft-new", "New message", "", {});
mk("mail", "sn-0", "Hana Sato", "Re: Photos from the walk", { when: "Thu" }); mk("mail", "sn-1", "Eva Sato", "Dinner on Saturday?", { when: "Tue" }); mk("mail", "sn-2", "Dev Patel", "Re: Invoice 2026-113", { when: "12 Nov" });
const dates: Record<string, string> = { "m-0": "2026-11-20", "m-1": "2026-11-20", "m-2": "2026-11-19", "m-3": "2026-11-16", "m-4": "2026-11-15", "m-5": "2026-11-14", "mx-0": "2026-11-20", "mx-1": "2026-11-18", "mx-2": "2026-11-17", "mx-3": "2026-11-17", "mx-4": "2026-11-20", "mx-5": "2026-11-19" };
for (const [id, d] of Object.entries(dates)) ITEMS[id]!.date = d;
// Which conversation shows a message count and an unread dot comes from its messages.
for (const [id, t] of Object.entries(THREADS)) { const it = ITEMS[id]; if (it && t.msgs.length > 1) it.n = t.msgs.length; }

const inbox = pick("mx-4", "mx-0", "m-0", "m-1", "m-2", "mx-5", "mx-1", "mx-3", "mx-2", "m-4", "m-5", "m-3");
def({ id: "mail-inbox", name: "Inbox", hue: 205, library: true, small: "list", big: "list", items: inbox });
def({ id: "mail-flagged", name: "Flagged", hue: 28, library: true, small: "list", big: "list", items: pick("m-0", "m-3", "mx-4") });
def({ id: "mail-drafts", name: "Drafts", hue: 150, small: "list", big: "list", items: pick("dr-0", "dr-1") });
def({ id: "mail-sent", name: "Sent", hue: 205, shut: true, small: "list", big: "list", items: pick("sn-0", "sn-1", "sn-2") });
def({ id: "mail-personal", name: "Personal: ada@lindqvist.example", hue: 175, shut: true, small: "list", big: "list", items: pick("m-0", "m-1", "m-2", "m-3", "m-4", "m-5", "mx-1", "mx-5") });
def({ id: "mail-studio", name: "Studio: ada@studio-lindqvist.example", hue: 280, shut: true, small: "list", big: "list", items: pick("mx-4", "mx-0", "mx-3") });
def({ id: "mail-receipts", name: "Receipts", hue: 150, shut: true, small: "list", big: "table", items: pick("m-0", "m-4", "m-5", "mx-1", "mx-3", "mx-8") });
def({ id: "mail-travel", name: "Travel", hue: 205, library: true, shut: true, small: "list", big: "list", items: pick("m-0", "m-1") });
