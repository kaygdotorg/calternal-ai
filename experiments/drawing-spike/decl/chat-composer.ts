// The official composer part, `calternal-chat-composer`. authoring.md declares the slot but writes out only a third-party
// composer (section 2.4); drawing.md 5.3 sketches this part as `composer({ send, attach, mention, draft })`. This is that
// sketch in authoring.md's shape, plus the binding of the send operation's inputs to what the slot gives (change D9).
import { plugin } from "./sdk.ts";
export default plugin({
  id: "calternal-chat-composer", name: "Chat composer", version: "2.0.0", icon: "edit",
  depends: [{ plugin: "calternal-chat", version: "^2" }],
  fills: { "calternal-chat/composer": { composer: { send: "calternal-chat/send", draft: true, mention: "this.people",
    inputs: { chat: "this", text: "draft", quote: "quote" } } } }, // D9
});
