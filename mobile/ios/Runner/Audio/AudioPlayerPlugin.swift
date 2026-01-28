import Flutter
import Foundation
import AVFoundation
import MediaPlayer

public class AudioPlayerPlugin: NSObject, FlutterPlugin, FlutterStreamHandler {
    private static let METHOD_CHANNEL = "com.pinepods/audio_player"
    private static let EVENT_CHANNEL = "com.pinepods/audio_events"

    private var eventSink: FlutterEventSink?
    private var audioPlayer: PinepodsAudioPlayer?
    private var carPlayContentManager: CarPlayContentManager?
    private var carPlayContentProvider: CarPlayContentProvider?

    public static func register(with registrar: FlutterPluginRegistrar) {
        let instance = AudioPlayerPlugin()

        let methodChannel = FlutterMethodChannel(
            name: METHOD_CHANNEL,
            binaryMessenger: registrar.messenger()
        )
        registrar.addMethodCallDelegate(instance, channel: methodChannel)

        let eventChannel = FlutterEventChannel(
            name: EVENT_CHANNEL,
            binaryMessenger: registrar.messenger()
        )
        eventChannel.setStreamHandler(instance)

        // Defer audio player initialization to avoid blocking the main thread
        // during app startup. AVAudioSession.setActive() can block on iOS.
        DispatchQueue.main.async {
            NSLog("[AudioPlayerPlugin] Initializing audio player (deferred)")
            instance.audioPlayer = PinepodsAudioPlayer(eventSink: instance.eventSink)

            // Initialize CarPlay support (only on real devices)
            instance.initializeCarPlay(methodChannel: methodChannel)
        }
    }

    private func initializeCarPlay(methodChannel: FlutterMethodChannel) {
        // Skip CarPlay initialization on simulator - MPPlayableContentManager
        // can cause issues and CarPlay isn't available anyway
        #if targetEnvironment(simulator)
        NSLog("[CarPlay] Skipping CarPlay initialization on simulator")
        return
        #else
        NSLog("[CarPlay] Initializing CarPlay support")

        carPlayContentManager = CarPlayContentManager(methodChannel: methodChannel)

        if let contentManager = carPlayContentManager {
            carPlayContentProvider = CarPlayContentProvider(contentManager: contentManager)

            let playableContentManager = MPPlayableContentManager.shared()
            playableContentManager.dataSource = carPlayContentProvider
            playableContentManager.delegate = carPlayContentProvider

            NSLog("[CarPlay] CarPlay content provider registered")
        }
        #endif
    }

    // MARK: - FlutterStreamHandler

    public func onListen(withArguments arguments: Any?, eventSink events: @escaping FlutterEventSink) -> FlutterError? {
        self.eventSink = events
        audioPlayer?.updateEventSink(events)
        return nil
    }

    public func onCancel(withArguments arguments: Any?) -> FlutterError? {
        self.eventSink = nil
        audioPlayer?.updateEventSink(nil)
        return nil
    }

    // MARK: - FlutterPlugin Method Handling

    public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        guard let player = audioPlayer else {
            result(FlutterError(code: "NOT_INITIALIZED", message: "Audio player not initialized", details: nil))
            return
        }

        switch call.method {
        case "playEpisode":
            handlePlayEpisode(call, player: player, result: result)

        case "play":
            player.play()
            result(nil)

        case "pause":
            player.pause()
            result(nil)

        case "stop":
            player.stop()
            result(nil)

        case "seek":
            if let args = call.arguments as? [String: Any],
               let position = args["position"] as? Int {
                player.seek(toMilliseconds: position)
                result(nil)
            } else {
                result(FlutterError(code: "INVALID_ARGS", message: "Position required", details: nil))
            }

        case "fastForward":
            let milliseconds = (call.arguments as? [String: Any])?["milliseconds"] as? Int ?? 30000
            player.fastForward(milliseconds: milliseconds)
            result(nil)

        case "rewind":
            let milliseconds = (call.arguments as? [String: Any])?["milliseconds"] as? Int ?? 10000
            player.rewind(milliseconds: milliseconds)
            result(nil)

        case "setPlaybackSpeed":
            if let args = call.arguments as? [String: Any],
               let speed = args["speed"] as? Double {
                player.setPlaybackSpeed(Float(speed))
                result(nil)
            } else {
                result(FlutterError(code: "INVALID_ARGS", message: "Speed required", details: nil))
            }

        case "getPosition":
            result(player.getCurrentPosition())

        case "getDuration":
            result(player.getDuration())

        // CarPlay browsing methods - handled by Flutter repository
        case "getSubscriptions",
             "getPodcastEpisodes",
             "getDownloads",
             "getQueue",
             "getRecent",
             "playFromMediaId",
             "search":
            // These are invoked by CarPlay and handled by Flutter
            // The result is passed back via the method channel callback
            result(nil)

        default:
            result(FlutterMethodNotImplemented)
        }
    }

    private func handlePlayEpisode(_ call: FlutterMethodCall, player: PinepodsAudioPlayer, result: @escaping FlutterResult) {
        guard let args = call.arguments as? [String: Any],
              let url = args["url"] as? String else {
            result(FlutterError(code: "INVALID_ARGS", message: "URL required", details: nil))
            return
        }

        let startPosition = args["startPosition"] as? Int ?? 0
        let isLocal = args["isLocal"] as? Bool ?? false
        let metadata = args["metadata"] as? [String: Any]

        player.playEpisode(
            url: url,
            startPosition: startPosition,
            isLocal: isLocal,
            metadata: metadata
        )
        result(nil)
    }
}

// MARK: - CarPlay Support

/// Manages CarPlay browsing content for Pinepods
/// Provides hierarchical podcast library access in CarPlay interface
class CarPlayContentManager: NSObject {
    private let methodChannel: FlutterMethodChannel

    // Root identifiers matching Android Auto structure
    static let rootIdentifier = "__ROOT__"
    static let subscriptionsIdentifier = "__SUBSCRIPTIONS__"
    static let downloadsIdentifier = "__DOWNLOADS__"
    static let queueIdentifier = "__QUEUE__"
    static let recentIdentifier = "__RECENT__"

    // Content ID prefixes
    static let podcastPrefix = "__PODCAST__|"
    static let episodePrefix = "__EPISODE__|"
    static let downloadPrefix = "__DOWNLOAD__|"
    static let queueItemPrefix = "__QUEUE__|"
    static let recentItemPrefix = "__RECENT__|"

    init(methodChannel: FlutterMethodChannel) {
        self.methodChannel = methodChannel
        super.init()
    }

    /// Get root menu items for CarPlay
    func getRootItems() -> [MPContentItem] {
        return [
            createBrowsableItem(
                identifier: Self.subscriptionsIdentifier,
                title: "Subscriptions",
                subtitle: "Your podcast subscriptions"
            ),
            createBrowsableItem(
                identifier: Self.downloadsIdentifier,
                title: "Downloads",
                subtitle: "Downloaded episodes"
            ),
            createBrowsableItem(
                identifier: Self.queueIdentifier,
                title: "Queue",
                subtitle: "Up next"
            ),
            createBrowsableItem(
                identifier: Self.recentIdentifier,
                title: "Recent",
                subtitle: "Recently played"
            )
        ]
    }

    /// Get children for a given parent identifier
    func getChildren(for identifier: String, completion: @escaping ([MPContentItem]?) -> Void) {
        NSLog("[CarPlay] Getting children for: \(identifier)")

        switch identifier {
        case Self.rootIdentifier:
            completion(getRootItems())

        case Self.subscriptionsIdentifier:
            getSubscriptions(completion: completion)

        case Self.downloadsIdentifier:
            getDownloads(completion: completion)

        case Self.queueIdentifier:
            getQueue(completion: completion)

        case Self.recentIdentifier:
            getRecent(completion: completion)

        default:
            if identifier.hasPrefix(Self.podcastPrefix) {
                let podcastId = String(identifier.dropFirst(Self.podcastPrefix.count))
                getPodcastEpisodes(podcastId: podcastId, completion: completion)
            } else {
                NSLog("[CarPlay] Unknown identifier: \(identifier)")
                completion([])
            }
        }
    }

    /// Get user's podcast subscriptions
    private func getSubscriptions(completion: @escaping ([MPContentItem]?) -> Void) {
        methodChannel.invokeMethod("getSubscriptions", arguments: nil) { result in
            guard let podcasts = result as? [[String: Any]] else {
                NSLog("[CarPlay] Failed to get subscriptions")
                completion([])
                return
            }

            let items = podcasts.compactMap { podcast -> MPContentItem? in
                guard let id = podcast["id"] as? String,
                      let title = podcast["title"] as? String else {
                    return nil
                }

                let imageUrl = podcast["imageUrl"] as? String
                let episodeCount = podcast["episodeCount"] as? Int ?? 0

                return self.createBrowsableItem(
                    identifier: "\(Self.podcastPrefix)\(id)",
                    title: title,
                    subtitle: "\(episodeCount) episodes",
                    artworkUrl: imageUrl
                )
            }

            completion(items)
        }
    }

    /// Get episodes for a specific podcast
    private func getPodcastEpisodes(podcastId: String, completion: @escaping ([MPContentItem]?) -> Void) {
        methodChannel.invokeMethod("getPodcastEpisodes", arguments: ["podcastId": podcastId]) { result in
            guard let episodes = result as? [[String: Any]] else {
                NSLog("[CarPlay] Failed to get podcast episodes")
                completion([])
                return
            }

            let items = episodes.compactMap { episode -> MPContentItem? in
                return self.createPlayableEpisodeItem(from: episode, prefix: Self.episodePrefix)
            }

            completion(items)
        }
    }

    /// Get downloaded episodes
    private func getDownloads(completion: @escaping ([MPContentItem]?) -> Void) {
        methodChannel.invokeMethod("getDownloads", arguments: nil) { result in
            guard let episodes = result as? [[String: Any]] else {
                NSLog("[CarPlay] Failed to get downloads")
                completion([])
                return
            }

            let items = episodes.compactMap { episode -> MPContentItem? in
                return self.createPlayableEpisodeItem(from: episode, prefix: Self.downloadPrefix)
            }

            completion(items)
        }
    }

    /// Get queue
    private func getQueue(completion: @escaping ([MPContentItem]?) -> Void) {
        methodChannel.invokeMethod("getQueue", arguments: nil) { result in
            guard let episodes = result as? [[String: Any]] else {
                NSLog("[CarPlay] Failed to get queue")
                completion([])
                return
            }

            let items = episodes.enumerated().compactMap { index, episode -> MPContentItem? in
                return self.createPlayableEpisodeItem(from: episode, prefix: "\(Self.queueItemPrefix)\(index)|")
            }

            completion(items)
        }
    }

    /// Get recently played episodes
    private func getRecent(completion: @escaping ([MPContentItem]?) -> Void) {
        methodChannel.invokeMethod("getRecent", arguments: nil) { result in
            guard let episodes = result as? [[String: Any]] else {
                NSLog("[CarPlay] Failed to get recent")
                completion([])
                return
            }

            let items = episodes.compactMap { episode -> MPContentItem? in
                return self.createPlayableEpisodeItem(from: episode, prefix: Self.recentItemPrefix)
            }

            completion(items)
        }
    }

    /// Play an episode from its content identifier
    func playFromIdentifier(_ identifier: String) {
        NSLog("[CarPlay] Playing from identifier: \(identifier)")

        // Extract the episode GUID from the identifier
        let guid: String?

        if identifier.hasPrefix(Self.episodePrefix) {
            guid = String(identifier.dropFirst(Self.episodePrefix.count))
        } else if identifier.hasPrefix(Self.downloadPrefix) {
            guid = String(identifier.dropFirst(Self.downloadPrefix.count))
        } else if identifier.hasPrefix(Self.recentItemPrefix) {
            guid = String(identifier.dropFirst(Self.recentItemPrefix.count))
        } else if identifier.hasPrefix(Self.queueItemPrefix) {
            // Queue items have format: __QUEUE__|<index>|<guid>
            let parts = identifier.split(separator: "|")
            guid = parts.count >= 3 ? String(parts[2]) : nil
        } else {
            NSLog("[CarPlay] Cannot play non-episode identifier: \(identifier)")
            return
        }

        guard let episodeGuid = guid else {
            NSLog("[CarPlay] Failed to extract GUID from identifier")
            return
        }

        methodChannel.invokeMethod("playFromMediaId", arguments: ["guid": episodeGuid])
    }

    // MARK: - Helper Methods

    /// Create a browsable (folder) content item
    private func createBrowsableItem(identifier: String, title: String, subtitle: String?, artworkUrl: String? = nil) -> MPContentItem {
        let item = MPContentItem(identifier: identifier)
        item.isContainer = true
        item.isPlayable = false
        item.title = title
        item.subtitle = subtitle

        if let artworkUrl = artworkUrl, let url = URL(string: artworkUrl) {
            item.artwork = MPMediaItemArtwork(boundsSize: CGSize(width: 300, height: 300)) { size in
                // Placeholder - actual artwork loading would be async
                return UIImage()
            }
        }

        return item
    }

    /// Create a playable episode content item
    private func createPlayableEpisodeItem(from episode: [String: Any], prefix: String) -> MPContentItem? {
        guard let guid = episode["guid"] as? String,
              let title = episode["title"] as? String else {
            return nil
        }

        let podcast = episode["podcast"] as? String ?? "Unknown Podcast"
        let duration = episode["duration"] as? Int ?? 0
        let imageUrl = episode["imageUrl"] as? String

        let item = MPContentItem(identifier: "\(prefix)\(guid)")
        item.isContainer = false
        item.isPlayable = true
        item.title = title
        item.subtitle = podcast

        // Format duration
        if duration > 0 {
            let minutes = duration / 60
            let seconds = duration % 60
            item.playbackProgress = 0.0
        }

        if let artworkUrl = imageUrl, let url = URL(string: artworkUrl) {
            item.artwork = MPMediaItemArtwork(boundsSize: CGSize(width: 300, height: 300)) { size in
                // Placeholder - actual artwork loading would be async
                return UIImage()
            }
        }

        return item
    }
}

/// CarPlay content data source and delegate
/// Integrates with MPPlayableContentManager to provide browsing in CarPlay
class CarPlayContentProvider: NSObject, MPPlayableContentDataSource, MPPlayableContentDelegate {
    private let contentManager: CarPlayContentManager
    private var contentCache: [String: [MPContentItem]] = [:]
    private let cacheQueue = DispatchQueue(label: "com.pinepods.carplay.cache")

    init(contentManager: CarPlayContentManager) {
        self.contentManager = contentManager
        super.init()

        // Pre-cache root items
        contentCache[CarPlayContentManager.rootIdentifier] = contentManager.getRootItems()
    }

    // MARK: - MPPlayableContentDataSource

    func beginLoadingChildItems(at indexPath: IndexPath, completionHandler: @escaping (Error?) -> Void) {
        NSLog("[CarPlay] Begin loading children at indexPath: \(indexPath)")

        let identifier = identifierForIndexPath(indexPath)

        // Check cache first
        if cacheQueue.sync(execute: { contentCache[identifier] != nil }) {
            NSLog("[CarPlay] Using cached items for \(identifier)")
            completionHandler(nil)
            return
        }

        // Load from content manager
        contentManager.getChildren(for: identifier) { [weak self] items in
            guard let self = self else {
                completionHandler(NSError(domain: "com.pinepods", code: -1, userInfo: nil))
                return
            }

            if let items = items {
                self.cacheQueue.async {
                    self.contentCache[identifier] = items
                }
                NSLog("[CarPlay] Loaded \(items.count) items for \(identifier)")
                completionHandler(nil)
            } else {
                NSLog("[CarPlay] Failed to load items for \(identifier)")
                completionHandler(NSError(domain: "com.pinepods", code: -1, userInfo: nil))
            }
        }
    }

    func childItemsDisplayPlaybackProgress(at indexPath: IndexPath) -> Bool {
        // Show playback progress for playable items (episodes)
        let identifier = identifierForIndexPath(indexPath)
        return identifier != CarPlayContentManager.rootIdentifier
    }

    func numberOfChildItems(at indexPath: IndexPath) -> Int {
        let identifier = identifierForIndexPath(indexPath)

        return cacheQueue.sync {
            let count = contentCache[identifier]?.count ?? 0
            NSLog("[CarPlay] numberOfChildItems for \(identifier): \(count)")
            return count
        }
    }

    func contentItem(at indexPath: IndexPath) -> MPContentItem? {
        let parentIdentifier = identifierForIndexPath(indexPath.dropLast())
        let itemIndex = indexPath.last ?? 0

        return cacheQueue.sync {
            guard let items = contentCache[parentIdentifier], itemIndex < items.count else {
                NSLog("[CarPlay] Item not found at indexPath: \(indexPath)")
                return nil
            }
            return items[itemIndex]
        }
    }

    // MARK: - MPPlayableContentDelegate

    func playableContentManager(_ contentManager: MPPlayableContentManager, initiatePlaybackOfContentItemAt indexPath: IndexPath, completionHandler: @escaping (Error?) -> Void) {
        NSLog("[CarPlay] Initiate playback at indexPath: \(indexPath)")

        // Get the identifier and play
        let identifier = identifierForIndexPath(indexPath)
        self.contentManager.playFromIdentifier(identifier)

        completionHandler(nil)
    }

    func playableContentManager(_ contentManager: MPPlayableContentManager, didUpdate context: MPPlayableContentManagerContext) {
        NSLog("[CarPlay] Context updated - enforcedContentItemsCount: \(context.enforcedContentItemsCount)")
    }

    // MARK: - Helper Methods

    /// Convert IndexPath to content identifier
    private func identifierForIndexPath(_ indexPath: IndexPath) -> String {
        if indexPath.count == 0 {
            return CarPlayContentManager.rootIdentifier
        }

        // Build identifier by walking the index path
        var currentIdentifier = CarPlayContentManager.rootIdentifier
        var currentIndexPath = IndexPath()

        for index in indexPath {
            currentIndexPath.append(index)

            // Get the cached items for the current level
            guard let items = cacheQueue.sync(execute: { contentCache[currentIdentifier] }),
                  index < items.count else {
                NSLog("[CarPlay] Failed to resolve indexPath: \(indexPath) at level \(currentIndexPath)")
                return currentIdentifier
            }

            currentIdentifier = items[index].identifier
        }

        return currentIdentifier
    }
}
