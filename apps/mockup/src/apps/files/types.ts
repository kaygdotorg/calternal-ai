/**
 * The kinds of file the Files mockup knows how to draw: for each extension a label, a family (which look its thumbnail and preview take), a hue for its colour tile, and a glyph from the icon set.
 * One place per fact: the thumbnail, the list's Kind column, the preview page and the panel all read this table.
 */
export type Family = "folder" | "pdf" | "doc" | "text" | "sheet" | "slide" | "image" | "video" | "audio" | "archive" | "code";
export interface FileType { label: string; family: Family; hue: number; glyph: string; /** The short name drawn on the tile ("PDF"). */ tag: string }
const T = (label: string, family: Family, hue: number, glyph: string, tag: string): FileType => ({ label, family, hue, glyph, tag });
export const TYPES: Record<string, FileType> = {
  pdf: T("PDF document", "pdf", 25, "file", "PDF"), docx: T("Word document", "doc", 245, "file", "DOC"), md: T("Markdown", "text", 285, "notes", "MD"), txt: T("Plain text", "text", 260, "file", "TXT"),
  xlsx: T("Spreadsheet", "sheet", 152, "table", "XLS"), csv: T("CSV table", "sheet", 165, "table", "CSV"), pptx: T("Presentation", "slide", 52, "file", "PPT"),
  png: T("PNG image", "image", 330, "image", "PNG"), jpg: T("JPEG image", "image", 330, "image", "JPG"), heic: T("HEIC image", "image", 330, "image", "HEIC"),
  mov: T("Video", "video", 205, "play", "MOV"), mp4: T("Video", "video", 205, "play", "MP4"), m4a: T("Audio", "audio", 300, "play", "M4A"), mp3: T("Audio", "audio", 300, "play", "MP3"),
  zip: T("Archive", "archive", 85, "archive", "ZIP"), ts: T("TypeScript", "code", 215, "code", "TS"), json: T("JSON", "code", 190, "code", "JSON"),
};
export const FOLDER: FileType = { label: "Folder", family: "folder", hue: 235, glyph: "folder", tag: "" };
export const extOf = (name: string): string => (name.includes(".") ? name.slice(name.lastIndexOf(".") + 1).toLowerCase() : "");
export const typeOf = (name: string, folder = false): FileType => (folder ? FOLDER : (TYPES[extOf(name)] ?? T("Document", "text", 260, "file", "FILE")));
