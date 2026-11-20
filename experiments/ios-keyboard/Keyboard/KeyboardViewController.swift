import UIKit
import UniformTypeIdentifiers

/// The spike keyboard: letters, numbers, delete, return, globe, a dictation key and a strip
/// that shows clipboard history, or live dictation text while dictating.
///
/// Keys are drawn by the keyboard itself because iOS offers no system key control to a
/// keyboard extension; everything else (the input view's background, the globe menu,
/// UIPasteControl, input clicks) is the system's.
final class KeyboardViewController: UIInputViewController {
    private enum Page { case letters, numbers, symbols }

    private var page = Page.letters
    private var shifted = true
    private var capsLock = false
    private let rows = UIStackView()
    private let strip = StripView()
    private let dictation = KeyboardDictation()
    private var deleteTimer: Timer?
    private var lastSpace = Date.distantPast
    private var shiftTapAt = Date.distantPast

    override func viewDidLoad() {
        super.viewDidLoad()
        SpikeLog.write("keyboard loaded: fullAccess=\(hasFullAccess) \(SpikeLog.memory())")
        guard let input = inputView else { return }
        input.allowsSelfSizing = true

        rows.axis = .vertical
        rows.spacing = Tokens.rowGap
        rows.distribution = .fillEqually
        strip.translatesAutoresizingMaskIntoConstraints = false
        rows.translatesAutoresizingMaskIntoConstraints = false
        input.addSubview(strip)
        input.addSubview(rows)
        let height = Tokens.stripHeight + 4 * Tokens.rowHeight + 3 * Tokens.rowGap + 12
        let h = input.heightAnchor.constraint(equalToConstant: height)
        h.priority = .defaultHigh
        NSLayoutConstraint.activate([
            h,
            strip.topAnchor.constraint(equalTo: input.topAnchor),
            strip.leadingAnchor.constraint(equalTo: input.leadingAnchor, constant: Tokens.sideInset),
            strip.trailingAnchor.constraint(equalTo: input.trailingAnchor, constant: -Tokens.sideInset),
            strip.heightAnchor.constraint(equalToConstant: Tokens.stripHeight),
            rows.topAnchor.constraint(equalTo: strip.bottomAnchor, constant: 4),
            rows.leadingAnchor.constraint(equalTo: input.leadingAnchor, constant: Tokens.sideInset),
            rows.trailingAnchor.constraint(equalTo: input.trailingAnchor, constant: -Tokens.sideInset),
            rows.bottomAnchor.constraint(equalTo: input.bottomAnchor, constant: -8),
        ])

        strip.onInsert = { [weak self] text in self?.textDocumentProxy.insertText(text) }
        strip.pasteTarget = self
        pasteConfiguration = UIPasteConfiguration(acceptableTypeIdentifiers: [UTType.plainText.identifier, UTType.utf8PlainText.identifier])
        dictation.onChange = { [weak self] state, text in
            self?.strip.showDictation(state, text)
            self?.reloadKeys()
        }
        dictation.onText = { [weak self] text in self?.insertDictation(text) }
        reloadKeys()
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        if hasFullAccess { Spike.defaults.set(Date().timeIntervalSince1970, forKey: Spike.Key.fullAccessSeen) }
        captureClipboard()
        strip.showClips(ClipboardHistory.load(), fullAccess: hasFullAccess)
        updateShiftForContext()
    }

    override func textDidChange(_ textInput: (any UITextInput)?) {
        super.textDidChange(textInput)
        updateShiftForContext()
    }

    // MARK: Clipboard

    /// Reads the pasteboard once when it changed since the last look. `changeCount` and
    /// `hasStrings` read no content; `string` does, and is where iOS may ask to paste.
    private func captureClipboard() {
        guard hasFullAccess else {
            SpikeLog.write("clipboard: no Full Access, skipped")
            return
        }
        let board = UIPasteboard.general
        let count = board.changeCount
        let seen = Spike.defaults.integer(forKey: Spike.Key.pasteboardChangeCount)
        guard count != seen else { return }
        Spike.defaults.set(count, forKey: Spike.Key.pasteboardChangeCount)
        guard board.hasStrings else {
            SpikeLog.write("clipboard changed (\(count)), no text")
            return
        }
        let started = Date()
        let text = board.string
        SpikeLog.write("clipboard read in \(ms(started, Date())) ms: \(text == nil ? "nil" : "\(text!.count) chars")")
        if let text { ClipboardHistory.add(text, source: "keyboard") }
    }

    override func paste(itemProviders: [NSItemProvider]) {
        for provider in itemProviders where provider.canLoadObject(ofClass: NSString.self) {
            _ = provider.loadObject(ofClass: NSString.self) { [weak self] value, _ in
                guard let text = value as? String else { return }
                DispatchQueue.main.async {
                    SpikeLog.write("UIPasteControl gave \(text.count) chars")
                    ClipboardHistory.add(text, source: "paste-control")
                    self?.textDocumentProxy.insertText(text)
                    self?.strip.showClips(ClipboardHistory.load(), fullAccess: self?.hasFullAccess ?? false)
                }
            }
        }
    }

    // MARK: Dictation

    private func insertDictation(_ text: String) {
        let before = textDocumentProxy.documentContextBeforeInput ?? ""
        var out = text
        if let last = before.last, !last.isWhitespace { out = " " + out }
        if !before.isEmpty, !before.trimmingCharacters(in: .whitespaces).hasSuffix("."), let first = out.first(where: { !$0.isWhitespace }) {
            // Mid-sentence: lowercase the first word unless it is "I" or all capitals.
            let words = out.split(separator: " ", maxSplits: 1)
            if let w = words.first, w != "I", !w.hasPrefix("I'"), w.uppercased() != w, first.isUppercase {
                out = out.replacingOccurrences(of: String(w), with: w.lowercased(), options: [], range: out.range(of: String(w)))
            }
        }
        textDocumentProxy.insertText(out)
    }

    // MARK: Keys

    private func reloadKeys() {
        rows.arrangedSubviews.forEach { $0.removeFromSuperview() }
        let upper = shifted || capsLock
        func letters(_ s: String) -> [Key] { s.map { .char(upper ? $0.uppercased() : String($0)) } }
        let layout: [[Key]]
        switch page {
        case .letters:
            layout = [letters("qwertyuiop"), letters("asdfghjkl"), [.shift] + letters("zxcvbnm") + [.delete]]
        case .numbers:
            layout = ["1234567890".map { .char(String($0)) }, ["-", "/", ":", ";", "(", ")", "$", "&", "@", "\""].map { .char($0) },
                      [.symbols] + [".", ",", "?", "!", "'"].map { .char($0) } + [.delete]]
        case .symbols:
            layout = [["[", "]", "{", "}", "#", "%", "^", "*", "+", "="].map { .char($0) }, ["_", "\\", "|", "~", "<", ">", "€", "£", "¥", "•"].map { .char($0) },
                      [.numbersPage] + [".", ",", "?", "!", "'"].map { .char($0) } + [.delete]]
        }
        var bottom: [Key] = [page == .letters ? .numbersPage : .lettersPage]
        if needsInputModeSwitchKey { bottom.append(.globe) }
        bottom += [.dictate, .space, .returnKey]
        for (i, row) in (layout + [bottom]).enumerated() {
            let view = rowView(row, centered: page == .letters && i == 1)
            rows.addArrangedSubview(view)
        }
    }

    /// One row. Widths are in units of a tenth of the keyboard (`rows`) minus the gap: letters are
    /// one unit, the bottom row's side keys 1.25, return 2.25, space and shift/delete take the rest.
    private func rowView(_ keys: [Key], centered: Bool) -> UIView {
        let stack = UIStackView()
        stack.axis = .horizontal
        stack.spacing = Tokens.keyGap
        let container = UIView()
        container.addSubview(stack)
        stack.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: container.topAnchor),
            stack.bottomAnchor.constraint(equalTo: container.bottomAnchor),
            stack.centerXAnchor.constraint(equalTo: container.centerXAnchor),
        ])
        if !centered {
            NSLayoutConstraint.activate([
                stack.leadingAnchor.constraint(equalTo: container.leadingAnchor),
                stack.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            ])
        }
        func unit(_ b: UIView, _ n: CGFloat) {
            b.widthAnchor.constraint(equalTo: container.widthAnchor, multiplier: 0.1 * n, constant: -Tokens.keyGap * 0.9).isActive = true
        }
        var flexible: [KeyButton] = []
        for key in keys {
            let b = KeyButton(key: key, active: isActive(key))
            b.addTarget(self, action: #selector(tapped(_:)), for: .touchUpInside)
            b.addTarget(self, action: #selector(click), for: .touchDown)
            stack.addArrangedSubview(b)
            switch key {
            case .char: unit(b, 1)
            case .delete:
                b.addGestureRecognizer(UILongPressGestureRecognizer(target: self, action: #selector(deleteHeld(_:))))
                flexible.append(b)
            case .globe:
                b.addTarget(self, action: #selector(handleInputModeList(from:with:)), for: .allTouchEvents)
                unit(b, 1.25)
            case .numbersPage, .lettersPage, .dictate: unit(b, 1.25)
            case .returnKey: unit(b, 2.25)
            case .shift, .symbols: flexible.append(b)
            case .space: b.setContentHuggingPriority(.defaultLow, for: .horizontal)
            }
        }
        if flexible.count == 2 { flexible[0].widthAnchor.constraint(equalTo: flexible[1].widthAnchor).isActive = true }
        return container
    }

    private func isActive(_ key: Key) -> Bool {
        switch key {
        case .shift: shifted || capsLock
        case .dictate: dictation.state == .recording || dictation.state == .transcribing
        default: false
        }
    }

    @objc private func click() {
        UIDevice.current.playInputClick()
    }

    @objc private func tapped(_ sender: KeyButton) {
        let proxy = textDocumentProxy
        switch sender.key {
        case .char(let s):
            proxy.insertText(s)
            if shifted && !capsLock {
                shifted = false
                reloadKeys()
            }
            if page != .letters, s == "'" {
                page = .letters
                reloadKeys()
            }
        case .space:
            // Double space: ". " as on the system keyboard.
            if Date().timeIntervalSince(lastSpace) < 0.4, let before = proxy.documentContextBeforeInput, before.hasSuffix(" "), before.dropLast().last?.isLetter == true {
                proxy.deleteBackward()
                proxy.insertText(". ")
            } else {
                proxy.insertText(" ")
            }
            lastSpace = Date()
            if page != .letters {
                page = .letters
                reloadKeys()
            }
        case .delete: proxy.deleteBackward()
        case .returnKey: proxy.insertText("\n")
        case .shift:
            if Date().timeIntervalSince(shiftTapAt) < 0.3 {
                capsLock = true
                shifted = true
            } else {
                capsLock = false
                shifted.toggle()
            }
            shiftTapAt = Date()
            reloadKeys()
        case .numbersPage: page = .numbers; reloadKeys()
        case .lettersPage: page = .letters; reloadKeys()
        case .symbols: page = .symbols; reloadKeys()
        case .globe: break
        case .dictate: dictation.toggle(host: view, opener: self)
        }
    }

    @objc private func deleteHeld(_ g: UILongPressGestureRecognizer) {
        switch g.state {
        case .began:
            deleteTimer = Timer.scheduledTimer(withTimeInterval: 0.08, repeats: true) { [weak self] _ in
                self?.textDocumentProxy.deleteBackward()
            }
        case .ended, .cancelled, .failed:
            deleteTimer?.invalidate()
            deleteTimer = nil
        default: break
        }
    }

    private func updateShiftForContext() {
        guard !capsLock, page == .letters else { return }
        let before = textDocumentProxy.documentContextBeforeInput ?? ""
        let trimmed = before.trimmingCharacters(in: .whitespaces)
        let want = before.isEmpty || before.hasSuffix("\n") || ((trimmed.hasSuffix(".") || trimmed.hasSuffix("?") || trimmed.hasSuffix("!")) && before.hasSuffix(" "))
        if want != shifted {
            shifted = want
            reloadKeys()
        }
    }
}

/// One key's meaning.
enum Key: Equatable {
    case char(String)
    case shift, delete, space, returnKey, globe, dictate, numbersPage, lettersPage, symbols
}

/// A key: a `UIButton` styled from `Tokens`.
final class KeyButton: UIButton {
    let key: Key

    init(key: Key, active: Bool) {
        self.key = key
        super.init(frame: .zero)
        var c = UIButton.Configuration.filled()
        c.background.cornerRadius = Tokens.keyRadius
        c.contentInsets = .init(top: 0, leading: 2, bottom: 0, trailing: 2)
        let special: Bool
        switch key {
        case .char(let s): c.attributedTitle = AttributedString(s, attributes: .init([.font: Tokens.keyFont])); special = false
        case .space: c.attributedTitle = AttributedString("space", attributes: .init([.font: Tokens.specialFont])); special = false
        case .returnKey: c.attributedTitle = AttributedString("return", attributes: .init([.font: Tokens.specialFont])); special = true
        case .numbersPage: c.attributedTitle = AttributedString("123", attributes: .init([.font: Tokens.specialFont])); special = true
        case .lettersPage: c.attributedTitle = AttributedString("ABC", attributes: .init([.font: Tokens.specialFont])); special = true
        case .symbols: c.attributedTitle = AttributedString("#+=", attributes: .init([.font: Tokens.specialFont])); special = true
        case .shift: c.image = UIImage(systemName: active ? "shift.fill" : "shift"); special = !active
        case .delete: c.image = UIImage(systemName: "delete.left"); special = true
        case .globe: c.image = UIImage(systemName: "globe"); special = true
        case .dictate: c.image = UIImage(systemName: active ? "stop.fill" : "mic.fill"); special = true
        }
        c.baseForegroundColor = key == .dictate ? (active ? .white : Tokens.accent) : Tokens.keyText
        c.baseBackgroundColor = key == .dictate && active ? Tokens.recording : (special ? Tokens.specialKeyFill : Tokens.keyFill)
        configuration = c
        layer.shadowColor = Tokens.keyShadow.cgColor
        layer.shadowOffset = CGSize(width: 0, height: 1)
        layer.shadowOpacity = 1
        layer.shadowRadius = 0
        accessibilityLabel = {
            switch key {
            case .char(let s): s
            case .dictate: active ? "Stop dictation" : "Dictate"
            case .globe: "Next keyboard"
            default: c.attributedTitle.map { String($0.characters) } ?? "\(key)"
            }
        }()
    }

    required init?(coder: NSCoder) { fatalError() }
}

/// The strip above the keys: clipboard chips (with a system Paste control), or dictation text.
final class StripView: UIView {
    var onInsert: (String) -> Void = { _ in }
    weak var pasteTarget: UIResponder? {
        didSet { paste.target = pasteTarget }
    }

    private let scroll = UIScrollView()
    private let chips = UIStackView()
    private let label = UILabel()
    private let paste: UIPasteControl = {
        let c = UIPasteControl.Configuration()
        c.displayMode = .iconOnly
        c.cornerStyle = .capsule
        c.baseBackgroundColor = Tokens.accent
        c.baseForegroundColor = .white
        return UIPasteControl(configuration: c)
    }()

    override init(frame: CGRect) {
        super.init(frame: frame)
        scroll.showsHorizontalScrollIndicator = false
        chips.axis = .horizontal
        chips.spacing = 6
        chips.alignment = .center
        label.font = Tokens.stripFont
        label.textColor = Tokens.stripText
        label.lineBreakMode = .byTruncatingHead
        label.isHidden = true
        [scroll, label].forEach {
            $0.translatesAutoresizingMaskIntoConstraints = false
            addSubview($0)
        }
        chips.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(chips)
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: leadingAnchor), scroll.trailingAnchor.constraint(equalTo: trailingAnchor),
            scroll.topAnchor.constraint(equalTo: topAnchor), scroll.bottomAnchor.constraint(equalTo: bottomAnchor),
            chips.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 4),
            chips.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -4),
            chips.centerYAnchor.constraint(equalTo: scroll.centerYAnchor),
            chips.heightAnchor.constraint(equalToConstant: 32),
            label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8), label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            label.centerYAnchor.constraint(equalTo: centerYAnchor),
            paste.widthAnchor.constraint(equalToConstant: 44),
            paste.heightAnchor.constraint(equalToConstant: 32),
        ])
    }

    required init?(coder: NSCoder) { fatalError() }

    func showClips(_ items: [ClipItem], fullAccess: Bool) {
        label.isHidden = true
        scroll.isHidden = false
        chips.arrangedSubviews.forEach { $0.removeFromSuperview() }
        chips.addArrangedSubview(paste)
        if !fullAccess {
            let note = UILabel()
            note.text = "Allow Full Access for clipboard history"
            note.font = Tokens.stripFont
            note.textColor = Tokens.stripText
            chips.addArrangedSubview(note)
        }
        for item in items.prefix(12) {
            var c = UIButton.Configuration.tinted()
            c.cornerStyle = .capsule
            c.baseForegroundColor = Tokens.accent
            c.baseBackgroundColor = Tokens.accent
            let short = item.text.replacingOccurrences(of: "\n", with: " ")
            c.attributedTitle = AttributedString(short.count > 28 ? String(short.prefix(28)) + "…" : short, attributes: .init([.font: Tokens.stripFont]))
            let b = UIButton(configuration: c, primaryAction: UIAction { [weak self] _ in self?.onInsert(item.text) })
            chips.addArrangedSubview(b)
        }
    }

    func showDictation(_ state: KeyboardDictation.State, _ text: String) {
        if case .idle = state {
            showClips(ClipboardHistory.load(), fullAccess: true)
            return
        }
        scroll.isHidden = true
        label.isHidden = false
        label.text = text
        label.textColor = state == .recording ? Tokens.keyText : Tokens.stripText
        if case .message = state {
            DispatchQueue.main.asyncAfter(deadline: .now() + 6) { [weak self] in
                guard let self, self.label.text == text else { return }
                self.showClips(ClipboardHistory.load(), fullAccess: true)
            }
        }
    }
}
