/**
 * Invented content for the Files previews: the first lines a thumbnail shows, the pages of a PDF, the text of a document (one of them long, to show a scrolling page), and a spreadsheet's cells. Generic generators fill in files that have no text of their own.
 */
import { rng } from "../photos/scene";

/** The first lines of a document, for the page thumbnail. */
export const BODY: Record<string, string> = {
  "Lease 2026.pdf": "Residential tenancy agreement between the landlord and the tenant for the flat at Rua das Flores 12, for twelve months from 1 January 2026.",
  "Itinerary.pdf": "Saturday 14 November. Flight to Lisbon at 07:10, taxi to Alfama, check in after 14:00. Saturday: Belém and the pastry shop, dinner at eight.",
  "Flight tickets.pdf": "Boarding pass. Porto to Lisbon, Saturday 14 November, seat 12A. Gate closes twenty minutes before departure.",
  "Hotel booking.pdf": "Hotel Alfama. Two nights, one double room, breakfast included. Check-in from 14:00; ask at the desk for the roof terrace key.",
  "Passport scan.pdf": "Scanned copy, pages 1 and 2. Keep offline. Do not share the link outside the household.",
  "Invoice 0412.pdf": "Invoice 0412, issued 8 November. Design review, three sessions. Payable within fourteen days to the account below.",
  "Insurance policy.pdf": "Household contents policy. Cover for fire, water damage and theft up to the sum insured. Excess of one hundred and fifty euros.",
  "Form 1040 draft.pdf": "Draft return for the tax year 2026. Figures are provisional until the accountant has checked the deductions and the foreign income schedule.",
  "Keys handover.pdf": "Record of keys handed over on 16 October: two front door keys, one cellar key, one mailbox key, one remote.",
  "November pick.pdf": "Chapter one. The ferry left the quay a little before six, and by then most of the passengers had already given up on the weather.",
  "Contract draft.docx": "Services agreement, draft three. The supplier will deliver the design system in two stages, with a review after each.",
  "Résumé.docx": "Product designer with ten years in small teams. Interfaces, systems and the small details that make an app feel finished.",
  "Accountant letter.docx": "Dear Ada, thank you for the documents. Three receipts are missing a date, and the train tickets need the booking reference.",
  "Utility transfer.docx": "Please move the electricity, water and internet accounts to the new address from 1 November. Meter readings are enclosed.",
  "Discussion questions.docx": "1. Which character changes most, and what causes it? 2. Is the ferry a place or a metaphor? 3. Who tells the truth?",
  "Calternal rewrite plan.md": "The plan for the three regions: web first, then the Mac app, then the phone. Each region ships a vertical slice before it widens.",
  "Autumn trip.md": "Flights are booked and the hotel is still open. Check the tram pass and the museum hours before Friday.",
  "Colemak, week three.md": "Typing speed is back to 62 words a minute after the second week. Home row first, then the pinky stretches.",
  "Dal makhani.md": "Soak the beans overnight. Eight hours on low, then cream and butter at the end. Serve with rice or warm bread.",
  "Reading list.md": "Affordances and signifiers. A door that needs a sign has failed. Notes on the second chapter and what to try next.",
  "Packing list.md": "Passport, charger, adapter, a light jacket, walking shoes. The tram pass is in the green folder.",
  "Standup notes.md": "Monday: the sidebar spacing is settled. Tuesday: gallery view needs a denser grid. Blocked: nothing.",
  "Moving checklist.md": "Change address with the bank, cancel the old internet, label the boxes by room, return the keys.",
  "Timetable.txt": "07:10 flight. 08:25 land. 09:30 taxi to Alfama. 14:00 check in. 20:00 dinner reservation.",
};
export interface Block { t: "h" | "p" | "li" | "kv"; s: string; v?: string }
const LEASE: Block[][] = [
  [{ t: "h", s: "Residential tenancy agreement" }, { t: "p", s: "This agreement is made on 1 December 2025 between Femi Hansen (the landlord) and Ada Lindqvist (the tenant) for the flat at Rua das Flores 12, Porto." },
    { t: "h", s: "1. Term" }, { t: "p", s: "The tenancy begins on 1 January 2026 and runs for twelve months. It renews for a further twelve months unless either party gives sixty days' notice in writing." },
    { t: "h", s: "2. Rent" }, { t: "kv", s: "Monthly rent", v: "€1,150.00" }, { t: "kv", s: "Due on", v: "the 1st of the month" }, { t: "kv", s: "Deposit", v: "€2,300.00" }, { t: "p", s: "Rent is paid by bank transfer to the account named in schedule A. A payment more than ten days late may be charged interest at the legal rate." }],
  [{ t: "h", s: "3. Use of the property" }, { t: "p", s: "The tenant will use the flat as a private home only and will not sublet it without the landlord's written consent. Pets are allowed with notice." },
    { t: "h", s: "4. Repairs and maintenance" }, { t: "p", s: "The landlord keeps the structure, the roof and the shared stair in good repair. The tenant looks after the inside of the flat and reports damage promptly." }, { t: "li", s: "Report leaks and electrical faults the same day." }, { t: "li", s: "Keep the smoke alarms working and change the batteries each spring." }, { t: "li", s: "Do not drill into the tiled walls of the bathroom." }],
  [{ t: "h", s: "5. Utilities and charges" }, { t: "p", s: "Electricity, water, gas and the internet are in the tenant's name. The building's shared charge of €38 a month is paid with the rent." },
    { t: "h", s: "6. Ending the tenancy" }, { t: "p", s: "At the end the tenant returns the flat clean and in the condition recorded in the inventory, allowing for fair wear. The deposit is returned within thirty days." }],
  [{ t: "h", s: "7. Notices" }, { t: "p", s: "Notices are sent by email to the addresses in schedule B and, for the end of the tenancy, also by registered post." }, { t: "h", s: "8. Governing law" }, { t: "p", s: "This agreement is governed by Portuguese law. Disputes go first to mediation and then to the courts of Porto." }],
  [{ t: "h", s: "Schedule A: payment details" }, { t: "kv", s: "Account holder", v: "Femi Hansen" }, { t: "kv", s: "Reference", v: "RF 2026 01" }, { t: "h", s: "Schedule B: contact" }, { t: "kv", s: "Landlord", v: "femi@example.org" }, { t: "kv", s: "Tenant", v: "ada@example.org" }],
  [{ t: "h", s: "Signatures" }, { t: "p", s: "Signed in two copies, one for each party, on the date at the top of this agreement." }, { t: "kv", s: "Landlord", v: "Femi Hansen" }, { t: "kv", s: "Tenant", v: "Ada Lindqvist" }],
];
const FILLER = ["The figures in this section are provisional and will be confirmed once the remaining documents arrive.", "A short summary of what changed since the last version, and what is still open.", "Everything on this page is fixture text, standing in for the real thing so the layout can be judged.", "The second table lists the amounts by month, with the running total in the last column.", "Please keep this document with the others in the same folder and share it only with the people named above."];
/** The pages of a PDF: the lease has its own text, any other gets a title, the first lines of its thumbnail and filler, seeded by its name so it never changes. */
export function pdfPages(name: string, pages: number): Block[][] {
  if (name === "Lease 2026.pdf") return LEASE.slice(0, pages);
  const r = rng([...name].reduce((a, c) => a + c.charCodeAt(0), 7));
  return Array.from({ length: pages }, (_, i) => i === 0 ? [{ t: "h", s: name.replace(/\.pdf$/, "") }, { t: "p", s: BODY[name] ?? FILLER[0]! }, { t: "p", s: FILLER[Math.floor(r() * FILLER.length)]! }, { t: "kv", s: "Date", v: "8 November 2026" }, { t: "kv", s: "Reference", v: `RF ${1000 + Math.floor(r() * 8000)}` }, { t: "p", s: FILLER[Math.floor(r() * FILLER.length)]! }] as Block[]
    : [{ t: "h", s: `Section ${i + 1}` }, { t: "p", s: FILLER[Math.floor(r() * FILLER.length)]! }, { t: "p", s: FILLER[Math.floor(r() * FILLER.length)]! }, { t: "li", s: "First point, kept short." }, { t: "li", s: "Second point, a little longer than the first." }]);
}
const SECTIONS = ["Goals", "The web region", "The Mac app", "The phone", "Shared core", "Plugins and their pages", "Sync and storage", "Search", "Accounts and spaces", "Sharing", "Performance gates", "What we are not building", "Risks", "Order of work"];
/** The text of a document or note. The rewrite plan is long on purpose (fourteen sections), to judge a page that scrolls; the others are a few paragraphs. */
export function docBlocks(name: string): Block[] {
  const long = name === "Calternal rewrite plan.md", r = rng([...name].reduce((a, c) => a + c.charCodeAt(0), 3)), out: Block[] = [];
  out.push({ t: "p", s: BODY[name] ?? FILLER[0]! });
  const n = long ? SECTIONS.length : 3;
  for (let i = 0; i < n; i++) {
    out.push({ t: "h", s: long ? SECTIONS[i]! : ["Overview", "Details", "Next steps"][i]! }, { t: "p", s: FILLER[Math.floor(r() * FILLER.length)]! + " " + FILLER[Math.floor(r() * FILLER.length)]! });
    if (i % 2 === 0) out.push({ t: "li", s: "Decide the shape of the first slice." }, { t: "li", s: "Write down what must not change." }, { t: "li", s: "Check it on a phone before moving on." });
  }
  return out;
}
/** A spreadsheet's cells: a header and rows. The budget is a real-looking one; others get a generic table. */
export function sheetRows(name: string): { head: string[]; rows: string[][] } {
  if (name.startsWith("Budget")) return { head: ["Category", "Jan", "Feb", "Mar", "Apr", "Total"], rows: [["Rent", "1,150", "1,150", "1,150", "1,150", "4,600"], ["Groceries", "412", "388", "430", "401", "1,631"], ["Transport", "64", "64", "64", "64", "256"], ["Utilities", "148", "152", "139", "120", "559"], ["Insurance", "85", "85", "85", "85", "340"], ["Eating out", "120", "96", "142", "110", "468"], ["Savings", "500", "500", "500", "500", "2,000"]] };
  const r = rng(name.length * 31);
  return { head: ["Item", "Date", "Amount", "Category", "Note"], rows: Array.from({ length: 9 }, (_, i) => [["Train", "Café", "Hotel", "Museum", "Taxi", "Book", "Pharmacy", "Groceries", "Tickets"][i]!, `${String(2 + i * 3).padStart(2, "0")} Nov`, `${(5 + r() * 140).toFixed(2)}`, ["Travel", "Food", "Lodging", "Culture", "Travel"][i % 5]!, i % 3 ? "" : "Receipt attached"]) };
}
export const ARCHIVE_LIST = ["interviews/ada-lindqvist.m4a", "interviews/ben-okafor.m4a", "interviews/notes.md", "interviews/consent-forms.pdf", "summary.docx"];
