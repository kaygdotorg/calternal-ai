import SwiftUI

/// What an agent or the system is asking the person for: a question, an approval or a secret. They share one sticky bar above the composer; secret
/// requests expire (15 minutes), questions and approvals do not (`docs/plugin-api.md`).
public enum RequestKind: Sendable {
    case question, approval, secret

    var icon: IconName { switch self { case .question: .spark; case .approval: .shield; case .secret: .key } }
    var name: String { switch self { case .question: "Question"; case .approval: "Approval"; case .secret: "Secret" } }
}

/// The request bar: a glass card with the kind's icon, a title, the reason, an optional expiry line and the answers. `label` is required.
public struct RequestBar: View {
    private let label: String, kind: RequestKind, title: String, detail: String, expires: String?
    private let primary: (text: String, action: () -> Void), secondary: (text: String, action: () -> Void)?
    @Design private var d

    public init(label: String, kind: RequestKind, title: String, detail: String, expires: String? = nil,
                primary: (text: String, action: () -> Void), secondary: (text: String, action: () -> Void)? = nil) {
        (self.label, self.kind, self.title, self.detail, self.expires, self.primary, self.secondary) = (label, kind, title, detail, expires, primary, secondary)
    }

    public var body: some View {
        let m = d.metrics, t = d.theme
        VStack(alignment: .leading, spacing: m.space(4)) {
            HStack(alignment: .top, spacing: m.space(4)) {
                Icon(kind.icon, tier: .inline).foregroundStyle(t.accent)
                VStack(alignment: .leading, spacing: m.space(1)) {
                    Text(title).textRole(.ui, weight: .strong).foregroundStyle(t.ink)
                    Text(detail).textRole(.label).foregroundStyle(t.ink3)
                    if let expires { Text(expires).textRole(.section).foregroundStyle(t.ink3) }
                }
                Spacer(minLength: 0)
                Badge(text: kind.name, tone: .accent)
            }
            HStack(spacing: m.space(3)) {
                Spacer(minLength: 0)
                if let secondary { ActionButton(text: secondary.text, variant: .secondary, action: secondary.action) }
                ActionButton(text: primary.text, variant: .primary, action: primary.action)
            }
        }
        .padding(m.space(5))
        .glassSurface(RoundedRectangle(cornerRadius: m.radiusCard, style: .continuous))
        .accessibilityElement(children: .contain)
        .accessibilityLabel(label)
    }
}
