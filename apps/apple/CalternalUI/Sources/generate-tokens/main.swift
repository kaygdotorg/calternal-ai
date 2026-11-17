// Writes (or with --check, verifies) Sources/CalternalUI/Generated/Tokens.generated.swift from the web design system's token file.
// Usage: generate-tokens [--check] <tokens.json> <native-ui-study.json> <output.swift>   (scripts/tokens.sh passes the repository's paths)
import Foundation
import TokenGenerator

let args = Array(CommandLine.arguments.dropFirst())
let check = args.first == "--check"
let paths = args.filter { $0 != "--check" }
guard paths.count == 3 else {
    FileHandle.standardError.write(Data("usage: generate-tokens [--check] <tokens.json> <native-ui-study.json> <output.swift>\n".utf8))
    exit(64)
}
do {
    let source = try TokenGenerator.generate(tokensJSON: Data(contentsOf: URL(fileURLWithPath: paths[0])),
                                             studyJSON: Data(contentsOf: URL(fileURLWithPath: paths[1])))
    let out = URL(fileURLWithPath: paths[2])
    if check {
        let current = (try? String(contentsOf: out, encoding: .utf8)) ?? ""
        if current != source {
            FileHandle.standardError.write(Data("stale: \(paths[2]) differs from the token file. Run apps/apple/CalternalUI/scripts/tokens.sh\n".utf8))
            exit(1)
        }
        print("tokens: up to date")
    } else {
        try FileManager.default.createDirectory(at: out.deletingLastPathComponent(), withIntermediateDirectories: true)
        try source.write(to: out, atomically: true, encoding: .utf8)
        print("tokens: wrote \(source.utf8.count) bytes to \(paths[2])")
    }
} catch {
    FileHandle.standardError.write(Data("tokens: \(error)\n".utf8))
    exit(1)
}
