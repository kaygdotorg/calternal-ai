import SwiftUI

extension View {
    /// Fades the view's content out over `top` and `bottom` points at its edges: no hard cut-off and no solid band (`docs/ui-shell.md`).
    /// Cost: one offscreen mask pass over the view; keep it on the scroll container, not on every row.
    func edgeFade(top: CGFloat, bottom: CGFloat) -> some View {
        mask {
            VStack(spacing: 0) {
                LinearGradient(colors: [.clear, .black], startPoint: .top, endPoint: .bottom).frame(height: top)
                Rectangle()
                LinearGradient(colors: [.black, .clear], startPoint: .top, endPoint: .bottom).frame(height: bottom)
            }
        }
    }
}
