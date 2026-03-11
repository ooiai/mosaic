import Domain
import Foundation

public actor DesktopArchiveStore: DesktopPersistenceStoring {
    private let archiveURL: URL
    private let encoder: JSONEncoder
    private let decoder: JSONDecoder

    public init(
        archiveURL: URL = DesktopArchiveStore.defaultArchiveURL(),
        encoder: JSONEncoder = DesktopArchiveStore.makeEncoder(),
        decoder: JSONDecoder = DesktopArchiveStore.makeDecoder()
    ) {
        self.archiveURL = archiveURL
        self.encoder = encoder
        self.decoder = decoder
    }

    public func loadArchive() async -> DesktopArchive {
        guard let data = try? Data(contentsOf: archiveURL) else {
            return DesktopArchive()
        }
        return (try? decoder.decode(DesktopArchive.self, from: data)) ?? DesktopArchive()
    }

    public func saveArchive(_ archive: DesktopArchive) async throws {
        let data = try encoder.encode(archive)
        try FileManager.default.createDirectory(
            at: archiveURL.deletingLastPathComponent(),
            withIntermediateDirectories: true,
            attributes: nil
        )
        try data.write(to: archiveURL, options: [.atomic])
    }

    public static func defaultArchiveURL() -> URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSTemporaryDirectory())
        return base
            .appendingPathComponent("MosaicMacApp", isDirectory: true)
            .appendingPathComponent("desktop-archive.json")
    }

    public static func makeEncoder() -> JSONEncoder {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.dateEncodingStrategy = .iso8601
        return encoder
    }

    public static func makeDecoder() -> JSONDecoder {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return decoder
    }
}

public actor InMemoryDesktopArchiveStore: DesktopPersistenceStoring {
    private var archive: DesktopArchive

    public init(archive: DesktopArchive = .init()) {
        self.archive = archive
    }

    public func loadArchive() async -> DesktopArchive {
        archive
    }

    public func saveArchive(_ archive: DesktopArchive) async throws {
        self.archive = archive
    }
}
