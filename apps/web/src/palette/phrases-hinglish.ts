// Hinglish day words (C21): kal, aaj, parso, agle hafte. A language pack adds words, never a second parser:
// the English rules read them like any other day. "kal" means yesterday or tomorrow in Hindi; the Composer
// takes tomorrow because a new task is about what is ahead (change it here if kayg decides otherwise).
import { addDays, followingWeek } from "./dates";
import { addDayWord } from "./date-words";

addDayWord("aaj", (t) => t);
addDayWord("aj", (t) => t);
addDayWord("kal", (t) => addDays(t, 1));
addDayWord("parso", (t) => addDays(t, 2));
addDayWord("parson", (t) => addDays(t, 2));
addDayWord("agle hafte", (t) => followingWeek(1, t));
addDayWord("agle hafta", (t) => followingWeek(1, t));
