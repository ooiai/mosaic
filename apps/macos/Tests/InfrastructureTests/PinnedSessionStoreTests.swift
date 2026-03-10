import Domain
import Infrastructure
import XCTest

final class PinnedSessionStoreTests: XCTestCase {
    func testStorePersistsPerWorkspace() async {
        let suiteName = "PinnedSessionStoreTests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defaults.removePersistentDomain(forName: suiteName)
        let store = PinnedSessionStore(defaults: defaults)
        let workspaceA = UUID()
        let workspaceB = UUID()

        await store.setPinnedSessionID("thread-1", pinned: true, workspaceID: workspaceA)
        await store.setPinnedSessionID("thread-2", pinned: true, workspaceID: workspaceA)
        await store.setPinnedSessionID("thread-b", pinned: true, workspaceID: workspaceB)
        await store.setPinnedSessionID("thread-1", pinned: false, workspaceID: workspaceA)

        let workspaceAEntries = await store.pinnedSessionIDs(for: workspaceA)
        let workspaceBEntries = await store.pinnedSessionIDs(for: workspaceB)

        XCTAssertEqual(workspaceAEntries, ["thread-2"])
        XCTAssertEqual(workspaceBEntries, ["thread-b"])
    }
}
