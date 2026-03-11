import Domain
import Features
import XCTest

final class WorkbenchStateMappingTests: XCTestCase {
    func testPreviewFixturesProvideRenderableArchive() {
        XCTAssertEqual(PreviewFixtures.projectArchive.project.id, PreviewFixtures.project.id)
        XCTAssertEqual(PreviewFixtures.projectArchive.sessions.first?.id, PreviewFixtures.session.id)
        XCTAssertTrue(PreviewFixtures.projectArchive.messages.contains(where: { $0.kind == .task }))
        XCTAssertEqual(PreviewFixtures.projectSnapshot.sessions.first?.id, PreviewFixtures.session.id)
    }
}
