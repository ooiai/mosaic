import Infrastructure
import XCTest

final class CommandHistoryStoreTests: XCTestCase {
    func testRecordDeduplicatesAndPreservesMostRecentOrder() async {
        let suiteName = "CommandHistoryStoreTests-\(UUID().uuidString)"
        UserDefaults(suiteName: suiteName)!.removePersistentDomain(forName: suiteName)
        let store = CommandHistoryStore(defaults: UserDefaults(suiteName: suiteName)!, maxEntries: 4)

        await store.recordCommandActionID("refresh-workspace")
        await store.recordCommandActionID("new-thread")
        await store.recordCommandActionID("refresh-workspace")

        let history = await store.recentCommandActionIDs()

        XCTAssertEqual(history, ["refresh-workspace", "new-thread"])

        UserDefaults(suiteName: suiteName)!.removePersistentDomain(forName: suiteName)
    }

    func testRecordCapsHistoryLength() async {
        let suiteName = "CommandHistoryStoreTests-\(UUID().uuidString)"
        UserDefaults(suiteName: suiteName)!.removePersistentDomain(forName: suiteName)
        let store = CommandHistoryStore(defaults: UserDefaults(suiteName: suiteName)!, maxEntries: 3)

        await store.recordCommandActionID("one")
        await store.recordCommandActionID("two")
        await store.recordCommandActionID("three")
        await store.recordCommandActionID("four")

        let history = await store.recentCommandActionIDs()

        XCTAssertEqual(history, ["four", "three", "two"])

        UserDefaults(suiteName: suiteName)!.removePersistentDomain(forName: suiteName)
    }
}
