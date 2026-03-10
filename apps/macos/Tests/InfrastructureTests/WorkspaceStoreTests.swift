import Domain
import Infrastructure
import XCTest

final class WorkspaceStoreTests: XCTestCase {
    func testSaveDeduplicatesExistingWorkspaceByPath() async {
        let original = WorkspaceReference(
            id: UUID(uuidString: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")!,
            name: "mosaic",
            path: "/tmp/mosaic"
        )
        let replacement = WorkspaceReference(
            id: UUID(uuidString: "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb")!,
            name: "mosaic-renamed",
            path: "/tmp/./mosaic"
        )
        let store = InMemoryWorkspaceStore(workspaces: [original], selectedID: original.id)

        await store.save(workspace: replacement)

        let workspaces = await store.recentWorkspaces()
        XCTAssertEqual(workspaces.count, 1)
        XCTAssertEqual(workspaces.first?.name, "mosaic-renamed")
    }

    func testSelectUpdatesMostRecentWorkspaceOrdering() async {
        let older = WorkspaceReference(
            id: UUID(uuidString: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")!,
            name: "older",
            path: "/tmp/older",
            lastOpenedAt: Date(timeIntervalSince1970: 10)
        )
        let newer = WorkspaceReference(
            id: UUID(uuidString: "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb")!,
            name: "newer",
            path: "/tmp/newer",
            lastOpenedAt: Date(timeIntervalSince1970: 20)
        )
        let defaults = UserDefaults(suiteName: #function)!
        defaults.removePersistentDomain(forName: #function)
        let store = WorkspaceStore(defaults: defaults)

        await store.save(workspace: older)
        await store.save(workspace: newer)
        await store.select(workspaceID: older.id)

        let workspaces = await store.recentWorkspaces()
        XCTAssertEqual(workspaces.first?.id, older.id)
    }
}
