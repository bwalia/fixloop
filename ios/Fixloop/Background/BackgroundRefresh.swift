import BackgroundTasks
import Foundation
import UserNotifications

/// Checks GitHub in the background and posts a local notification when a new Fixloop review lands.
/// Runs entirely on device: no Fixloop server sees your token.
enum BackgroundRefresh {
    static let taskID = "run.fixloop.app.refresh"

    static func schedule() {
        let req = BGAppRefreshTaskRequest(identifier: taskID)
        req.earliestBeginDate = Date(timeIntervalSinceNow: 30 * 60)
        try? BGTaskScheduler.shared.submit(req)
    }

    static func requestNotificationPermission() async -> Bool {
        (try? await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .badge, .sound])) ?? false
    }

    /// Returns the number of new reviews found.
    @discardableResult
    static func run() async -> Int {
        schedule()
        guard let token = Keychain.load() else { return 0 }
        guard let prs = try? await InboxLoader(client: GitHubClient(token: token)).load(includeClosed: false) else { return 0 }
        let fresh = SeenStore.unseen(prs)
        SeenStore.markSeen(prs)
        for pr in fresh.prefix(5) {
            let content = UNMutableNotificationContent()
            content.title = "Fixloop reviewed \(pr.repo)#\(pr.number)"
            content.body = "\(pr.title): \(pr.summary.headline)"
            content.sound = .default
            content.userInfo = ["url": pr.url.absoluteString]
            let req = UNNotificationRequest(identifier: SeenStore.key(for: pr), content: content, trigger: nil)
            try? await UNUserNotificationCenter.current().add(req)
        }
        return fresh.count
    }
}
