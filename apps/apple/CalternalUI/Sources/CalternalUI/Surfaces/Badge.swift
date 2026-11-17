import SwiftUI

/// A one to three word status. The tone is never the only signal: `text` says it too, so a colour-blind reader loses nothing.
public struct Badge: View {
    private let text: String, tone: Tone, dot: Bool
    @Design private var d

    public init(text: String, tone: Tone = .neutral, dot: Bool = false) { (self.text, self.tone, self.dot) = (text, tone, dot) }

    public var body: some View {
        let c = d.theme.colour(tone)
        HStack(spacing: d.metrics.space(2)) {
            if dot { Circle().fill(c).frame(width: d.metrics.space(3), height: d.metrics.space(3)).accessibilityHidden(true) }
            Text(text).textRole(.section, weight: .strong).lineLimit(1)
        }
        .foregroundStyle(c)
        .padding(.horizontal, d.metrics.space(4)).frame(height: d.metrics.space(7))
        .background(c.opacity(0.14), in: Capsule())
        .accessibilityElement(children: .combine)
    }
}
