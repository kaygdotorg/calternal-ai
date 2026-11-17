import SwiftUI
import UIKit

/// Shows the safe-area insets and every text style's real point size and line height as accessibility text,
/// so a UI test can read them. Values come from the system (UIFont.preferredFont), not from a table.
struct ReportView: View {
    @Environment(\.dynamicTypeSize) var dts
    var body: some View {
        GeometryReader { g in
            let s = g.safeAreaInsets
            let tc = UITraitCollection(preferredContentSizeCategory: UIApplication.shared.preferredContentSizeCategory)
            let styles: [(String, UIFont.TextStyle)] = [("largeTitle", .largeTitle), ("title1", .title1), ("title2", .title2), ("title3", .title3), ("headline", .headline), ("body", .body), ("callout", .callout), ("subheadline", .subheadline), ("footnote", .footnote), ("caption1", .caption1), ("caption2", .caption2)]
            let lines = styles.map { (n, st) -> String in
                let f = UIFont.preferredFont(forTextStyle: st, compatibleWith: tc)
                return "\(n) size=\(f.pointSize) line=\(f.lineHeight)"
            }
            let text = "safe top=\(s.top) leading=\(s.leading) bottom=\(s.bottom) trailing=\(s.trailing) screen=\(UIScreen.main.bounds.size) scale=\(UIScreen.main.scale) cat=\(UIApplication.shared.preferredContentSizeCategory.rawValue)\n" + lines.joined(separator: "\n")
            Text(text).font(.system(size: 9, design: .monospaced)).accessibilityIdentifier("report").accessibilityLabel(text)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading).padding(8)
        }
    }
}

@main struct HostApp: App {
    var body: some Scene { WindowGroup { ReportView() } }
}
