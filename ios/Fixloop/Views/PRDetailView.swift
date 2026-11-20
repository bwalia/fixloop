import SwiftUI

struct PRDetailView: View {
    @Environment(AppModel.self) private var model
    let pr: ReviewedPR

    @State private var findings: [Finding] = []
    @State private var loading = true
    @State private var error: String?

    var body: some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    Text(pr.title).font(.title3.bold())
                    Text(verbatim: "\(pr.repo) #\(pr.number) · by \(pr.author)")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    Text(pr.summary.headline).font(.subheadline)
                    if let m = pr.summary.model {
                        Text("Reviewed with \(m)").font(.caption).foregroundStyle(.secondary)
                    }
                }
                .padding(.vertical, 4)
                Link(destination: pr.url) {
                    Label("Open on GitHub", systemImage: "arrow.up.forward.app")
                }
            }

            if loading {
                Section { HStack { Spacer(); ProgressView(); Spacer() } }
            } else if let error {
                Section { Text(error).foregroundStyle(.red) }
            } else if findings.isEmpty {
                Section {
                    Text(pr.summary.total == 0
                         ? "Fixloop found no problems in this pull request."
                         : "The inline comments for this review aren't available. Open the pull request on GitHub to see them.")
                        .foregroundStyle(.secondary)
                }
            } else {
                ForEach(Severity.allCases) { s in
                    let group = findings.filter { $0.severity == s }
                    if !group.isEmpty {
                        Section(s.label) {
                            ForEach(group) { f in
                                NavigationLink(value: f) { FindingRow(finding: f) }
                            }
                        }
                    }
                }
            }
        }
        .navigationTitle(Text(verbatim: "#\(pr.number)"))
        .navigationBarTitleDisplayMode(.inline)
        .navigationDestination(for: Finding.self) { FindingDetailView(finding: $0) }
        .task { await load() }
        .refreshable { await load() }
    }

    private func load() async {
        do {
            findings = try await model.findings(for: pr)
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
        loading = false
    }
}

struct FindingRow: View {
    let finding: Finding

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Image(systemName: finding.severity.symbol).foregroundStyle(finding.severity.color)
                Text(finding.title).font(.subheadline.weight(.semibold)).lineLimit(3)
            }
            Text(location)
                .font(.caption.monospaced())
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .truncationMode(.head)
        }
        .padding(.vertical, 2)
    }

    private var location: String {
        finding.line.map { "\(finding.path):\($0)" } ?? finding.path
    }
}

struct FindingDetailView: View {
    let finding: Finding
    @State private var copied = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                HStack(spacing: 8) {
                    SeverityBadge(severity: finding.severity)
                    if !finding.category.isEmpty {
                        Text(finding.category)
                            .font(.caption.weight(.medium))
                            .padding(.horizontal, 8).padding(.vertical, 3)
                            .background(.quaternary, in: Capsule())
                    }
                    if let c = finding.confidence {
                        Text("\(c)% confidence").font(.caption).foregroundStyle(.secondary)
                    }
                }
                Text(finding.title).font(.title3.bold())
                Text(finding.line.map { "\(finding.path):\($0)" } ?? finding.path)
                    .font(.footnote.monospaced())
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)

                Text(markdown(finding.rationale))
                    .textSelection(.enabled)

                if let fix = finding.fix {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("Suggested fix").font(.headline)
                        Text(markdown(fix)).textSelection(.enabled)
                    }
                }

                if let s = finding.suggestion {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text("Replacement code").font(.headline)
                            Spacer()
                            Button(copied ? "Copied" : "Copy") {
                                UIPasteboard.general.string = s
                                copied = true
                            }
                            .font(.subheadline)
                        }
                        ScrollView(.horizontal) {
                            Text(s)
                                .font(.system(.footnote, design: .monospaced))
                                .textSelection(.enabled)
                                .padding(12)
                        }
                        .background(.quaternary.opacity(0.6), in: RoundedRectangle(cornerRadius: 10))
                        Text("Commit it from the comment on GitHub with one click.")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }

                if let url = finding.url {
                    Link(destination: url) {
                        Label("Open comment on GitHub", systemImage: "arrow.up.forward.app")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.large)
                }
            }
            .padding()
            .frame(maxWidth: 720, alignment: .leading)
            .frame(maxWidth: .infinity)
        }
        .navigationBarTitleDisplayMode(.inline)
    }

    private func markdown(_ s: String) -> AttributedString {
        (try? AttributedString(markdown: s, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(s)
    }
}
