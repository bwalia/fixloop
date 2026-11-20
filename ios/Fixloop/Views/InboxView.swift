import SwiftUI

struct InboxView: View {
    @Environment(AppModel.self) private var model
    @State private var filter: Severity?
    @State private var showSettings = false

    private var rows: [ReviewedPR] {
        guard let filter else { return model.inbox }
        return model.inbox.filter { ($0.summary.counts[filter] ?? 0) > 0 }
    }

    var body: some View {
        @Bindable var model = model
        NavigationStack {
            List {
                if !model.inbox.isEmpty {
                    Section {
                        ScrollView(.horizontal, showsIndicators: false) {
                            HStack(spacing: 8) {
                                chip(nil, "All")
                                ForEach(Severity.allCases) { s in chip(s, s.label) }
                            }
                            .padding(.vertical, 2)
                        }
                        .listRowInsets(EdgeInsets(top: 4, leading: 16, bottom: 4, trailing: 16))
                        .listRowBackground(Color.clear)
                    }
                }
                Section {
                    ForEach(rows) { pr in
                        NavigationLink(value: pr) { PRRow(pr: pr) }
                    }
                }
            }
            .listStyle(.insetGrouped)
            .navigationTitle("Reviews")
            .navigationDestination(for: ReviewedPR.self) { PRDetailView(pr: $0) }
            .refreshable { await model.refresh() }
            .overlay { overlay }
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Menu {
                        Toggle("Include closed pull requests", isOn: $model.includeClosed)
                    } label: {
                        Image(systemName: "line.3.horizontal.decrease.circle")
                            .accessibilityLabel("Filter")
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button { showSettings = true } label: {
                        Image(systemName: "gearshape").accessibilityLabel("Settings")
                    }
                }
            }
            .sheet(isPresented: $showSettings) { SettingsView() }
            .task { await model.refresh() }
            .alert("Couldn't load reviews", isPresented: Binding(
                get: { model.errorMessage != nil }, set: { if !$0 { model.errorMessage = nil } }
            )) {
                Button("OK", role: .cancel) {}
            } message: {
                Text(model.errorMessage ?? "")
            }
        }
    }

    @ViewBuilder private var overlay: some View {
        if model.inbox.isEmpty {
            if model.isLoading {
                ProgressView("Loading reviews…")
            } else {
                ContentUnavailableView {
                    Label("No Fixloop reviews yet", systemImage: "checkmark.bubble")
                } description: {
                    Text("Pull requests you're involved in will show up here once Fixloop has reviewed them. Pull down to refresh.")
                } actions: {
                    Link("Set up Fixloop on a repository", destination: Brand.website)
                }
            }
        } else if rows.isEmpty, let filter {
            ContentUnavailableView("No \(filter.rawValue) findings", systemImage: "line.3.horizontal.decrease.circle")
        }
    }

    private func chip(_ s: Severity?, _ title: String) -> some View {
        let on = filter == s
        return Button { filter = s } label: {
            Text(title)
                .font(.subheadline.weight(on ? .semibold : .regular))
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .background(on ? Brand.accent : Color.secondary.opacity(0.12), in: Capsule())
                .foregroundStyle(on ? Color.white : Color.primary)
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

struct PRRow: View {
    let pr: ReviewedPR

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Image(systemName: pr.isOpen ? "arrow.triangle.pull" : "checkmark.circle")
                    .foregroundStyle(pr.isOpen ? Brand.accent : .secondary)
                Text(verbatim: "\(pr.repo) #\(pr.number)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                Spacer()
                if let date = pr.summary.submittedAt {
                    Text(date, format: .relative(presentation: .named))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            Text(pr.title)
                .font(.headline)
                .lineLimit(2)
            if pr.summary.total == 0 {
                Label("No problems found", systemImage: "checkmark.seal.fill")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(Brand.accent)
            } else {
                HStack(spacing: 6) {
                    ForEach(Severity.allCases.filter { (pr.summary.counts[$0] ?? 0) > 0 }) { s in
                        SeverityBadge(severity: s, count: pr.summary.counts[s])
                    }
                }
            }
        }
        .padding(.vertical, 4)
    }
}
