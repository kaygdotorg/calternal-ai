import SwiftUI
import CalternalUI

/// The generated-UI renderer on both fixtures, with state kept locally.
struct GenUIScene: View {
    let palette: WallpaperFixtures.Palette?
    @State private var question = LocalGenHost(state: GenUIFixtures.questionSnapshot.state)
    @State private var gallery = LocalGenHost(state: GenUIFixtures.gallerySnapshot.state)
    @State private var sent = ""
    @Design private var d

    var body: some View {
        WallpaperScene(palette: palette) {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    Card(label: "Question", variant: .solid) { GenUIView(roots: GenUIFixtures.questionSnapshot.roots, state: question.state, host: question).padding(20) }
                    Card(label: "Gallery", variant: .solid) { GenUIView(roots: GenUIFixtures.gallerySnapshot.roots, state: gallery.state, host: gallery).padding(20) }
                    Text(sent.isEmpty ? "Nothing sent yet." : "Sent: \(sent)").textRole(.label).foregroundStyle(d.theme.ink3).accessibilityIdentifier("genui.sent")
                }
                .padding(20).frame(maxWidth: 560).frame(maxWidth: .infinity)
            }
        }
        .onAppear { question.onSend = { sent = $0 } }
    }
}
