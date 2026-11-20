import BackgroundTasks
import SwiftUI

@main
struct FixloopApp: App {
    @State private var model = AppModel()
    @Environment(\.scenePhase) private var scenePhase

    init() {
        BGTaskScheduler.shared.register(forTaskWithIdentifier: BackgroundRefresh.taskID, using: nil) { task in
            let work = Task { await BackgroundRefresh.run() }
            task.expirationHandler = { work.cancel() }
            Task {
                _ = await work.value
                task.setTaskCompleted(success: true)
            }
        }
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(model)
                .tint(Brand.accent)
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background { BackgroundRefresh.schedule() }
        }
    }
}

struct RootView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        switch model.phase {
        case .signedOut: SignInView()
        case .signedIn: InboxView()
        }
    }
}

enum Brand {
    static let accent = Color("AccentColor")
    static let support = URL(string: "mailto:support@workstation.co.uk")!
    static let website = URL(string: "https://fixloop.run")!
    static let privacy = URL(string: "https://fixloop.run/privacy/")!
    static let source = URL(string: "https://github.com/bwalia/fixloop")!
}
