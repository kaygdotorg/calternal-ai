/**
 * Fixture people, shared by the Contacts, Mail and Calendar mockups (Mail's senders, Calendar's guests and the contact page's recent mail, chats and events all point at the same people). Invented names and addresses on `.example` domains; nothing here is real.
 * A person is a `contact` item (so the shell's rows, cards and cells draw it) plus the details the contact page shows.
 */
import { ITEMS, mk, type Item } from "../../data/collections";

export interface Person {
  /** The contact item's id (`cp-N`). */ id: string; name: string;
  /** One line under the name: what the person is to me. */ role: string; org?: string;
  phones: [label: string, value: string][]; emails: [label: string, value: string][];
  /** Postal address, one line each. */ address?: string[]; /** Day and month only; the year is often unknown. */ birthday?: string; /** ISO date of the next birthday, for the calendar's Birthdays. */ birthdayOn?: string;
  notes?: string; /** Group ids (`g-family`, `g-work`, `g-neighbours`, `g-services`). */ groups: string[]; fav?: boolean; me?: boolean;
  /** Ids of items elsewhere that mention this person: mail, chats, events. The page draws them as three small collections. */ mail: string[]; chats: string[]; events: string[];
}

const raw: Omit<Person, "id">[] = [
  { name: "Ada Lindqvist", role: "My card", phones: [["mobile", "+351 900 000 101"]], emails: [["home", "ada@lindqvist.example"], ["studio", "ada@studio-lindqvist.example"]], address: ["Rua das Flores 24, 3º", "1200-195 Lisboa"], birthday: "9 Jun", groups: [], me: true, mail: ["sn-0", "sn-1"], chats: ["c-ada"], events: ["e-0"] },
  { name: "Anna Okafor", role: "Accountant", org: "Okafor & Daughters", phones: [["work", "+351 900 000 142"]], emails: [["work", "anna@okafor-daughters.example"]], address: ["Av. da Liberdade 110, 4º", "1250-146 Lisboa"], notes: "Wants receipts as one PDF per month. Closed on Fridays after 13:00.", groups: ["g-work", "g-services"], mail: ["mx-1", "m-4"], chats: ["c-paper"], events: ["ev-10"] },
  { name: "Ben Martin", role: "Friend", phones: [["mobile", "+351 900 000 143"]], emails: [["home", "ben@martin.example"]], birthday: "2 Feb", groups: [], fav: true, mail: [], chats: ["c-ben"], events: ["ev-5"] },
  { name: "Chloe Rossi", role: "Colleague", org: "Studio Norte", phones: [["mobile", "+351 900 000 144"], ["work", "+351 210 000 144"]], emails: [["work", "chloe@studio-norte.example"]], groups: ["g-work"], fav: true, mail: ["mx-4"], chats: ["c-chloe"], events: ["e-2", "ev-4"] },
  { name: "Dev Patel", role: "Colleague", org: "Studio Norte", phones: [["mobile", "+351 900 000 145"]], emails: [["work", "dev@studio-norte.example"]], groups: ["g-work"], mail: ["mx-3"], chats: ["c-dev"], events: ["ev-4"] },
  { name: "Eva Sato", role: "Cousin", phones: [["mobile", "+351 900 000 146"]], emails: [["home", "eva@sato.example"]], birthday: "24 Nov", birthdayOn: "2026-11-24", groups: ["g-family"], fav: true, mail: ["sn-1"], chats: ["c-eva"], events: ["ev-8", "ev-3"] },
  {
    name: "Femi Hansen", role: "Landlord", org: "Hansen Imóveis", phones: [["mobile", "+351 900 000 147"], ["office", "+351 210 000 147"], ["after hours", "+351 900 000 148"]],
    emails: [["personal", "femi@hansen-imoveis.example"], ["office", "lettings@hansen-imoveis.example"], ["accounts", "accounts@hansen-imoveis.example"]],
    address: ["Rua das Flores 24, r/c", "1200-195 Lisboa"], birthday: "19 Aug",
    notes: "Owns the flat and the two next door. Prefers email for anything about money and a phone call for anything urgent. The lease renewal is due before the end of December; he mentioned a possible rent change of about three per cent and said he would send the paperwork by mid-month. Keys: one spare with Lena Berg, one with the office. The boiler is serviced every March, the man comes on a Tuesday and Femi tells us the week before.",
    groups: ["g-services", "g-neighbours"], mail: ["m-3", "dr-0", "mx-6", "mx-7", "mx-8"], chats: ["c-flat"], events: ["ev-6", "ev-11", "ev-12"],
  },
  { name: "Hana Sato", role: "Host in Lisbon", org: "Casa Sato", phones: [["mobile", "+351 900 000 149"]], emails: [["home", "hana@sato.example"]], address: ["Travessa do Poço 7", "1100-123 Lisboa"], birthday: "3 Apr", groups: ["g-family"], fav: true, mail: ["m-2", "sn-0"], chats: ["c-hana"], events: ["ev-2"] },
  { name: "Ivo Novak", role: "Colleague", org: "Studio Norte", phones: [["mobile", "+351 900 000 150"]], emails: [["work", "ivo@studio-norte.example"]], groups: ["g-work"], fav: true, mail: ["mx-0"], chats: ["c-ivo"], events: ["e-2"] },
  { name: "Jun Park", role: "Friend", phones: [["mobile", "+351 900 000 151"]], emails: [["home", "jun@park.example"]], birthday: "11 Dec", groups: [], mail: [], chats: ["c-jun"], events: ["ev-5"] },
  { name: "Lena Berg", role: "Neighbour", phones: [["mobile", "+351 900 000 152"]], emails: [["home", "lena@berg.example"]], address: ["Rua das Flores 24, 2º", "1200-195 Lisboa"], birthday: "21 Nov", birthdayOn: "2026-11-21", notes: "Has the spare key. Waters the plants when we are away.", groups: ["g-neighbours"], mail: [], chats: [], events: ["ev-9"] },
  { name: "Marta Silva", role: "Front desk, Hotel Alfama", org: "Hotel Alfama", phones: [["hotel", "+351 210 000 153"]], emails: [["reservations", "reservas@hotel-alfama.example"]], address: ["Rua de São Miguel 31", "1100-544 Lisboa"], groups: ["g-services"], mail: ["m-1"], chats: [], events: ["e-1"] },
  { name: "Dr. Sofia Ferreira", role: "Dentist", org: "Clínica do Chiado", phones: [["clinic", "+351 210 000 154"]], emails: [["clinic", "marcacoes@clinica-chiado.example"]], address: ["Rua Garrett 63, 1º", "1200-203 Lisboa"], groups: ["g-services"], mail: ["mx-5"], chats: [], events: ["e-0"] },
  { name: "Rui", role: "", phones: [["mobile", "+351 900 000 155"]], emails: [], groups: [], mail: [], chats: [], events: [] },
];

/** The people by contact item id. */
export const PEOPLE: Record<string, Person> = {};
/** The contact items, in the order above (A to Z is the page's own sort). */
export const CONTACT_ITEMS: Item[] = raw.map((p, i) => {
  const id = `cp-${i}`, it = ITEMS[id] ?? mk("contact", id, p.name, p.role);
  it.sub = p.role || undefined; PEOPLE[id] = { id, ...p };
  return it;
});
/** Finds a person by name (Mail's senders, Calendar's guests). Unknown names have no card. */
export const personByName = (name: string): Person | undefined => Object.values(PEOPLE).find((p) => p.name === name);
/** The address a name writes from, or a plausible invented one. */
export const addrOf = (name: string): string => personByName(name)?.emails[0]?.[1] ?? `${name.toLowerCase().replace(/[^a-z]+/g, ".").replace(/^\.|\.$/g, "")}@example.org`;
