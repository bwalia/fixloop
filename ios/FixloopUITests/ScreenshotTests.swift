import XCTest

/// Walks the app with sample data and saves App Store screenshots to $SCREENSHOT_DIR.
final class ScreenshotTests: XCTestCase {
    @MainActor
    func testScreenshots() throws {
        let app = XCUIApplication()
        app.launchArguments = ["-sampleData"]
        app.launch()

        XCTAssertTrue(app.staticTexts["Add refund endpoint for partial captures"].waitForExistence(timeout: 10))
        snap("1-inbox")

        app.staticTexts["Add refund endpoint for partial captures"].tap()
        XCTAssertTrue(app.staticTexts["Refund amount is not checked against the captured amount"].waitForExistence(timeout: 10))
        snap("2-pull-request")

        app.staticTexts["Refund amount is not checked against the captured amount"].tap()
        XCTAssertTrue(app.staticTexts["Suggested fix"].waitForExistence(timeout: 10))
        snap("3-finding")

        app.navigationBars.buttons.element(boundBy: 0).tap()
        app.navigationBars.buttons.element(boundBy: 0).tap()
        app.buttons["Critical"].tap()
        snap("4-filter")
        app.buttons["All"].tap()

        app.buttons["Settings"].tap()
        XCTAssertTrue(app.staticTexts["Settings"].waitForExistence(timeout: 5))
        snap("5-settings")
    }

    @MainActor
    private func snap(_ name: String) {
        sleep(1)
        let shot = XCUIScreen.main.screenshot()
        let a = XCTAttachment(screenshot: shot)
        a.name = name
        a.lifetime = .keepAlways
        add(a)
        if let dir = ProcessInfo.processInfo.environment["SCREENSHOT_DIR"] {
            try? shot.pngRepresentation.write(to: URL(fileURLWithPath: dir).appendingPathComponent("\(name).png"))
        }
    }
}
