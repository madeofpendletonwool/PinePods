import UIKit
import Flutter

@available(iOS 13.0, *)
class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?

    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options connectionOptions: UIScene.ConnectionOptions) {
        guard let windowScene = scene as? UIWindowScene else { return }

        window = UIWindow(windowScene: windowScene)

        // Create FlutterViewController with default engine
        let controller = FlutterViewController(project: nil, nibName: nil, bundle: nil)

        // Register all plugins with the engine
        GeneratedPluginRegistrant.register(with: controller.engine)

        // Register native audio player plugin
        if let registrar = controller.engine.registrar(forPlugin: "AudioPlayerPlugin") {
            AudioPlayerPlugin.register(with: registrar)
        }

        // Register CarPlay Now Playing channel on this engine (critical - must be same engine as Flutter code)
        if let registrar = controller.engine.registrar(forPlugin: "CarPlayNowPlayingChannel") {
            CarPlayNowPlayingChannel.register(with: registrar)
            NSLog("[SceneDelegate] CarPlayNowPlayingChannel registered with main Flutter engine")
        }

        window?.rootViewController = controller
        window?.makeKeyAndVisible()
    }
}
