// One registration line per app (see registry.ts and the README). Import order is the app's place in the frecency tie-break, nothing more.
import { registerApp } from "./registry";
import chat from "./chat"; registerApp(chat);
import mail from "./mail"; registerApp(mail);
import calendar from "./calendar"; registerApp(calendar);
import contacts from "./contacts"; registerApp(contacts);
import { files } from "./files"; registerApp(files);
import { photos } from "./photos"; registerApp(photos);
import notes from "./notes"; registerApp(notes);
import tasks from "./tasks"; registerApp(tasks);
export {};
