import UIKit

/// The keyboard's theme: the one token file. Change a value here to restyle every key.
/// Colours give a light and a dark value; the keyboard follows the host's appearance.
enum Tokens {
    static let keyFill = dynamic(light: .white, dark: UIColor(white: 0.42, alpha: 1))
    static let specialKeyFill = dynamic(light: UIColor(red: 0.67, green: 0.70, blue: 0.75, alpha: 1), dark: UIColor(white: 0.27, alpha: 1))
    static let keyText = UIColor.label
    static let keyShadow = dynamic(light: UIColor(white: 0, alpha: 0.30), dark: UIColor(white: 0, alpha: 0.55))
    /// The Calternal accent: the dictation key and the clipboard chips.
    static let accent = UIColor(red: 0.36, green: 0.42, blue: 0.95, alpha: 1)
    static let recording = UIColor.systemRed
    static let stripText = UIColor.secondaryLabel

    static let keyRadius: CGFloat = 8.5
    static let keyFont = UIFont.systemFont(ofSize: 23, weight: .regular)
    static let specialFont = UIFont.systemFont(ofSize: 16, weight: .regular)
    static let stripFont = UIFont.systemFont(ofSize: 14, weight: .medium)

    static let keyGap: CGFloat = 6
    static let rowGap: CGFloat = 11
    static let rowHeight: CGFloat = 45
    static let stripHeight: CGFloat = 40
    static let sideInset: CGFloat = 3

    private static func dynamic(light: UIColor, dark: UIColor) -> UIColor {
        UIColor { $0.userInterfaceStyle == .dark ? dark : light }
    }
}
