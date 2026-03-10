import Infrastructure
import XCTest

final class CLIEnvelopeTests: XCTestCase {
    func testDecodesSuccessEnvelope() throws {
        let data = Data(
            """
            {
              "config_path": "/tmp/.mosaic/config.toml",
              "mode": "project",
              "ok": true,
              "profile": "default"
            }
            """.utf8
        )

        let decoded = try JSONDecoder().decode(CLIEnvelope<CLISetupPayload>.self, from: data)

        XCTAssertTrue(decoded.ok)
        XCTAssertEqual(decoded.payload.toDomain().profile, "default")
        XCTAssertEqual(decoded.payload.toDomain().mode, "project")
    }

    func testDecodesErrorEnvelope() throws {
        let data = Data(
            """
            {
              "error": {
                "code": "config_missing",
                "message": "run setup first",
                "exit_code": 2
              },
              "ok": false
            }
            """.utf8
        )

        let decoded = try JSONDecoder().decode(CLIErrorEnvelope.self, from: data)

        XCTAssertFalse(decoded.ok)
        XCTAssertEqual(decoded.error.code, "config_missing")
        XCTAssertEqual(decoded.error.exitCode, 2)
    }
}
