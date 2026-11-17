import SwiftUI

/// One thing that needs the person.
public struct AttentionItem: Identifiable, Equatable, Sendable {
    public let id: String
    public let icon: IconName
    public let title: String
    public let sub: String
    public init(id: String, icon: IconName, title: String, sub: String) { (self.id, self.icon, self.title, self.sub) = (id, icon, title, sub) }
}

/// The attention card: what needs the person, at most three items (a count badge says when there are more). A card because it asks for attention, drawn on
/// glass, absent when empty. A thread shown here is not also listed below it (the caller's rule). `open` receives the tapped item.
public struct AttentionCard: View {
    private let items: [AttentionItem], open: (AttentionItem) -> Void
    @Design private var d

    public init(items: [AttentionItem], open: @escaping (AttentionItem) -> Void) { (self.items, self.open) = (items, open) }

    public var body: some View {
        if !items.isEmpty {
            VStack(alignment: .leading, spacing: d.metrics.space(1)) {
                HStack(spacing: d.metrics.space(3)) {
                    Text("Needs you").textRole(.label, weight: .strong).foregroundStyle(d.theme.ink3)
                    if items.count > 3 { Badge(text: "\(items.count)") }
                }
                .padding(.horizontal, d.metrics.space(4)).padding(.top, d.metrics.space(3)).padding(.bottom, d.metrics.space(2))
                .accessibilityAddTraits(.isHeader)
                ForEach(items.prefix(3)) { item in
                    Row(title: item.title, sub: item.sub, icon: item.icon) { open(item) }
                }
            }
            .padding(d.metrics.space(3))
            .glassSurface(RoundedRectangle(cornerRadius: d.metrics.radiusCard, style: .continuous))
            .accessibilityElement(children: .contain)
            .accessibilityLabel("Needs you")
        }
    }
}
