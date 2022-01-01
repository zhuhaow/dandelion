//
//  Server.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/16.
//

import Foundation
import Defaults

struct ServerInformation {
    let socks5: Endpoint?
    let http: Endpoint?
}

enum ServerEvent {
    case beforeStarted(ServerInformation)
    case completed(Result<Void, String>)
}

// We are not using `actor` since it won't work with synchronous code (other than creating a `Task`).
// Before Apple makes AppKit fully async-ready, we cannot use the new fancy concurrecy feature with anything
// but the very limited SwiftUI.

// We don't really care the cycle reference here since the server instance is expected
// to exist through the app's lifetime.
//
// Call `stop()` can properly release all cycled reference.
class Server {
    private var stopHandle: StopHandle?
    private var afterStop: (() -> Void)?
    private let queue = DispatchQueue(label: "me.zhuhaow.Specht2")

    var socks5: Endpoint?
    var http: Endpoint?

    private var running: Bool {
        self.stopHandle != nil
    }

    func run(name: String, configUrl: URL, eventCallback: @escaping (ServerEvent) -> Void) {
        queue.async {
            if self.running {
                self.afterStop = {
                    self.runServer(name: name, configUrl: configUrl, eventCallback: eventCallback)
                }

                self.stop()
            } else {
                self.runServer(name: name, configUrl: configUrl, eventCallback: eventCallback)
            }
        }
    }

    private func runServer(name: String, configUrl: URL, eventCallback: @escaping (ServerEvent) -> Void) {
        // We only clear it here, so every new start of server will never use old callback.
        afterStop = nil
        stopHandle = startServer(configUrl: configUrl) { event in
            self.queue.async {
                switch event {
                case .beforeStarted(let serverInformation):
                    self.socks5 = serverInformation.socks5
                    self.http = serverInformation.http
                    eventCallback(event)
                case .completed:
                    self.stopHandle = nil
                    self.socks5 = nil
                    self.http = nil
                    eventCallback(event)
                    self.afterStop?()
                }
            }
        }
    }

    private func stop() {
        stopHandle = nil
    }

    func shutdown() {
        queue.async {
            self.afterStop = nil
            self.stopHandle = nil
        }
    }

    func isRunning() -> Bool {
        return queue.sync {
            return self.running
        }
    }
}

private class StopHandle {
    let handle: UnsafeMutableRawPointer

    init(handle: UnsafeMutableRawPointer) {
        self.handle = handle
    }

    deinit {
        specht2_stop(handle)
    }
}

private func startServer(configUrl: URL, closure: @escaping (ServerEvent) -> Void) -> StopHandle {
    let wrappedClosure = WrapClosure(closure: closure)
    let data = Unmanaged.passRetained(wrappedClosure).toOpaque()

    let beforeStartedCallback: @convention(c) (UnsafeMutableRawPointer, UnsafeMutablePointer<ServerInfo>) -> Void
        = { (_ userdata: UnsafeMutableRawPointer, _ serverInfo: UnsafeMutablePointer<ServerInfo>) in
            // We won't take ownership here.
            let wrappedClosure: WrapClosure<(ServerEvent) -> Void> =
                Unmanaged.fromOpaque(userdata).takeUnretainedValue()

            var socks5: Endpoint?
            if let addr = serverInfo.pointee.socks5_addr {
                socks5 = Endpoint(addr: String.init(cString: addr), port: serverInfo.pointee.socks5_port)
            }

            var http: Endpoint?
            if let addr = serverInfo.pointee.http_addr {
                http = Endpoint(addr: String.init(cString: addr), port: serverInfo.pointee.http_port)
            }

            let info = ServerInformation(socks5: socks5, http: http)

            wrappedClosure.closure(.beforeStarted(info))
        }

    let doneCallback: @convention(c) (UnsafeMutableRawPointer, UnsafePointer<CChar>?) -> Void
        = { (_ userdata: UnsafeMutableRawPointer, _ err: UnsafePointer<CChar>?) in
            let wrappedClosure: WrapClosure<(ServerEvent) -> Void> = Unmanaged.fromOpaque(userdata).takeRetainedValue()

            guard let err = err else {
                wrappedClosure.closure(.completed(.success(())))
                return
            }

            // The conversion should never fail
            wrappedClosure.closure(.completed(.failure(String.init(cString: err, encoding: .utf8)!)))
        }

    let completion = EventCallback(userdata: data,
                                   before_start_callback: beforeStartedCallback,
                                   done_callback: doneCallback)

    return StopHandle(handle: configUrl.withUnsafeFileSystemRepresentation {
        // Due to the limit of cbindgen, the generated signiture for NonNull is non-const,
        // thus the pointer has to be mutable.
        // But we won't mutate it anywhere.
        // The $0 should not be nil according to the docs since the path should always be
        // possible to have a valid representation.
        let mutablePtr = UnsafeMutablePointer(mutating: $0!)
        return specht2_start(mutablePtr, completion)
    })
}

private class WrapClosure<T> {
    let closure: T

    init(closure: T) {
        self.closure = closure
    }
}
