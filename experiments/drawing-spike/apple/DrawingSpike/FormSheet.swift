import SwiftUI

/// A form derived from an operation, as a system `Form` in a sheet: one field per input the action did not supply (labelled
/// and typed from the operation, or for a declarative create from the kind's fields), prefilled from the action's `defaults`,
/// and a picker for each input the action fills from a slot's parts. Submit is one call.
struct FormSheet: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    let action: Action, kind: String?, prefill: [String: String]
    @State private var values: [String: String] = [:]
    var body: some View {
        let op = store.op(action.op), fields = store.formFields(action, kind: kind)
        NavigationStack {
            Form {
                ForEach(fields, id: \.self) { f in
                    let b = Binding(get: { values[f.name] ?? "" }, set: { values[f.name] = $0 })
                    Section(f.label + (f.optional ? " (optional)" : "")) {
                        if f.type == "url" { TextField(f.label, text: b).keyboardType(.URL).textInputAutocapitalization(.never) }
                        else { TextField(f.label, text: b, axis: .vertical).lineLimit(1...8) }
                    }
                }
                ForEach(action.pick.sorted(by: { $0.key < $1.key }), id: \.key) { name, slot in
                    Picker(name.capitalized, selection: Binding(get: { values[name] ?? "" }, set: { values[name] = $0 })) {
                        ForEach(store.fillers(slot), id: \.self) { Text($0["name"].string ?? "").tag($0["plugin"].string ?? "") }
                    }
                }
            }
            .navigationTitle(op["title"].string ?? action.label).navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel", role: .cancel) { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button(op["title"].string ?? action.label, role: .confirm) { store.call(action.op, action.inputs.merging(values) { $1 }); dismiss() }
                        .disabled(fields.contains { !$0.optional && (values[$0.name] ?? "").isEmpty })
                }
            }
        }
        .presentationDetents([.medium, .large])
        .onAppear {
            values.merge(prefill) { $1 }
            for (name, slot) in action.pick where values[name] == nil { values[name] = store.fillers(slot).first?["plugin"].string }
        }
    }
}
