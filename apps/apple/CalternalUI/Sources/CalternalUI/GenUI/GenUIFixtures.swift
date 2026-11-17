import Foundation

/// Evaluations as `crates/genui` would hand them over (`Evaluation` serialised as JSON), for the preview app, tests and the shell's fixture message.
/// They are fixtures until the evaluator is exposed through CalternalCore (see the package README).
public enum GenUIFixtures {
    /// A question form: one `Choice`, one `Select`, a `Toggle`, a `Checklist`, free text and two buttons.
    public static let question = """
    {"state":{"format":"md","where":"space","keep":true,"extra":["links"],"note":""},
     "diagnostics":[],
     "roots":[{"kind":"node","type":"Form","key":"f","props":{"label":"Which export format should win?","children":[
       {"kind":"node","type":"Choice","key":"f.0","props":{"label":"Format","options":["md","json","html"],"value":{"kind":"binding","name":"format","value":"md"}}},
       {"kind":"node","type":"Select","key":"f.1","props":{"label":"Where it lives","options":[{"value":"space","label":"In the space"},{"value":"personal","label":"In my personal space"},{"value":"both","label":"Both"}],"value":{"kind":"binding","name":"where","value":"space"}}},
       {"kind":"node","type":"Toggle","key":"f.2","props":{"label":"Keep the old file","value":{"kind":"binding","name":"keep","value":true}}},
       {"kind":"node","type":"Checklist","key":"f.3","props":{"label":"Also export","options":["links","attachments","history"],"value":{"kind":"binding","name":"extra","value":["links"]}}},
       {"kind":"node","type":"Input","key":"f.4","props":{"label":"Note","placeholder":"Anything else?","value":{"kind":"binding","name":"note","value":""}}},
       {"kind":"node","type":"Row","key":"f.5","props":{"children":[
         {"kind":"node","type":"Button","key":"f.5.0","props":{"label":"Use Markdown","variant":"primary","action":[{"kind":"action","type":"send","text":"Markdown with frontmatter."}]}},
         {"kind":"node","type":"Button","key":"f.5.1","props":{"label":"Ask me later","action":[{"kind":"action","type":"send","text":"Ask me later."}]}}]}}]}}]}
    """

    /// Every component the native renderer draws, once.
    public static let gallery = """
    {"state":{"pick":"b","radio":"x","on":false,"list":[],"text":"","area":""},"diagnostics":[],
     "roots":[
      {"kind":"node","type":"Card","key":"c","props":{"title":"Run summary","children":[
        {"kind":"node","type":"Text","key":"c.0","props":{"text":"Ran **12** tests, `2` failed.","tone":"warn"}},
        {"kind":"node","type":"Badge","key":"c.1","props":{"text":"2 failed","tone":"bad"}},
        {"kind":"node","type":"Progress","key":"c.2","props":{"value":60,"max":100,"label":"Coverage","description":"60 percent of lines","tone":"good"}},
        {"kind":"node","type":"KeyValue","key":"c.3","props":{"pairs":{"Branch":"dev","Commit":"8dd51b4","Duration":"41 s"}}},
        {"kind":"node","type":"Divider","key":"c.4","props":{}},
        {"kind":"node","type":"Callout","key":"c.5","props":{"title":"Heads up","text":"The perf lock is held.","tone":"info"}},
        {"kind":"node","type":"Code","key":"c.6","props":{"code":"cargo test -p calternal-notes","language":"sh"}},
        {"kind":"node","type":"Details","key":"c.7","props":{"title":"Log","open":false,"children":[{"kind":"node","type":"Text","key":"c.7.0","props":{"text":"All other suites passed."}}]}},
        {"kind":"node","type":"Table","key":"c.8","props":{"columns":["Suite","Result"],"rows":[["notes","ok"],["store",{"kind":"node","type":"Badge","key":"c.8.r","props":{"text":"slow","tone":"warn"}}]]}}]}},
      {"kind":"node","type":"Stack","key":"s","props":{"children":[
        {"kind":"node","type":"Radio","key":"s.0","props":{"label":"Depth","options":[{"value":"x","label":"Quick","detail":"About a minute"},{"value":"y","label":"Thorough","detail":"Ten minutes"}],"value":{"kind":"binding","name":"radio","value":"x"}}},
        {"kind":"node","type":"TextArea","key":"s.1","props":{"label":"Notes","placeholder":"Longer text","value":{"kind":"binding","name":"area","value":""}}},
        {"kind":"node","type":"Chart","key":"s.2","props":{"label":"Trend","description":"A component this client does not draw yet"}}]}}]}
    """

    /// The question fixture, decoded once.
    public static let questionSnapshot = snapshot(question)
    /// The gallery fixture, decoded once.
    public static let gallerySnapshot = snapshot(gallery)

    /// Decodes a fixture.
    public static func snapshot(_ json: String) -> GenSnapshot { (try? GenSnapshot(json: Data(json.utf8))) ?? GenSnapshot(roots: [], state: [:]) }
}
