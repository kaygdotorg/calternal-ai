import SwiftUI

/// The native header over the top of the content card: the sidebar button, the title pill and the
/// trailing buttons, each a Liquid Glass control. Items come from `ChromeDescription.header`; the
/// sidebar button is the shell's own (it opens the sidebar without a round trip to the page).
struct HeaderView: View {
    let model: ChromeModel

    var body: some View {
        let header = model.description.header
        GlassEffectContainer(spacing: 10) {
            HStack(spacing: 10) {
                if model.description.sidebar != nil {
                    circle("sidebar.left", label: "Sidebar") {
                        model.setSidebarOpen(true)
                    }
                }
                ForEach(header?.leading ?? []) { item in itemView(item) }
                if let title = header?.title {
                    Button { model.send(ChromeEvent(type: "header", id: "title")) } label: {
                        Text(title).font(.headline).lineLimit(1).padding(.horizontal, 10).frame(minHeight: 36)
                    }
                    .buttonStyle(.glass)
                }
                Spacer(minLength: 0)
                ForEach(header?.trailing ?? []) { item in itemView(item) }
            }
        }
        .padding(.horizontal, 14)
        .preferredColorScheme(model.isDark ? .dark : .light)
    }

    @ViewBuilder
    private func itemView(_ item: ChromeDescription.Item) -> some View {
        if let menu = item.menu, !menu.isEmpty {
            Menu {
                ForEach(menu) { entry in
                    Button(role: entry.role == "destructive" ? .destructive : nil) {
                        model.send(ChromeEvent(type: "header", id: entry.id, rowId: item.id))
                    } label: { Label(entry.title, systemImage: entry.symbol ?? "circle") }
                        .disabled(entry.disabled == true)
                }
            } label: { Image(systemName: item.symbol ?? "ellipsis").font(.system(size: 18, weight: .medium)).frame(width: 36, height: 36) }
            .buttonStyle(.glass).buttonBorderShape(.circle).accessibilityLabel(item.title)
        } else {
            circle(item.symbol ?? "circle", label: item.title) { model.send(ChromeEvent(type: "header", id: item.id)) }
        }
    }

    private func circle(_ symbol: String, label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) { Image(systemName: symbol).font(.system(size: 18, weight: .medium)).frame(width: 36, height: 36) }
            .buttonStyle(.glass).buttonBorderShape(.circle).accessibilityLabel(label)
    }
}
