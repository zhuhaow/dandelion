//
//  Server.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/16.
//

import Foundation
import Defaults

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

    private var running: Bool {
        self.stopHandle != nil
    }

    func run(name: String, configUrl: URL, routeTraffic: Bool, doneCallback: @escaping (Result<Void, String>) -> Void) {
        queue.async {
            if self.running {
                self.afterStop = {
                    self.runServer(name: name,
                                   configUrl: configUrl,
                                   routeTraffic: routeTraffic,
                                   doneCallback: doneCallback)
                }

                self.stop()
            } else {
                self.runServer(name: name,
                               configUrl: configUrl,
                               routeTraffic: routeTraffic,
                               doneCallback: doneCallback)
            }
        }
    }

    private func runServer(name: String,
                           configUrl: URL,
                           routeTraffic: Bool,
                           doneCallback: @escaping (Result<Void, String>) -> Void) {
        // We only clear it here, so every new start of server will never use old callback.
        afterStop = nil
        stopHandle = startServer(configUrl: configUrl, routeTraffic: routeTraffic) { result in
            self.queue.async {
                self.stopHandle = nil
                doneCallback(result)
                self.afterStop?()
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

    func shutdownWith(semaphore: DispatchSemaphore) {
        queue.async {
            if self.running {
                self.afterStop = {
                    semaphore.signal()
                }
                self.stopHandle = nil
            } else {
                semaphore.signal()
            }
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

func setSocks5ProxyHandler(_: UnsafeMutableRawPointer,
                           serverInfo: UnsafeMutablePointer<ServerInfo>,
                           callback: ErrorCallback?,
                           callbackData: UnsafeMutableRawPointer) {
    let endpoint = convertServerInfoPtrToEndpoint(serverPtr: serverInfo)

    Task {
        do {
            let service = try await Service.getDefaultService()
            try await service.setSocks5Proxy(endpoint: endpoint)
            callback!(callbackData, nil)
        } catch {
            callback!(callbackData, error.localizedDescription)
        }
    }
}

func setHttpProxyHandler(_: UnsafeMutableRawPointer,
                         serverInfo: UnsafeMutablePointer<ServerInfo>,
                         callback: ErrorCallback?,
                         callbackData: UnsafeMutableRawPointer) {
    let endpoint = convertServerInfoPtrToEndpoint(serverPtr: serverInfo)

    Task {
        do {
            let service = try await Service.getDefaultService()
            try await service.setHttpProxy(endpoint: endpoint)
            callback!(callbackData, nil)
        } catch {
            callback!(callbackData, error.localizedDescription)
        }
    }
}

func setDnsHandler(_: UnsafeMutableRawPointer,
                   serverInfo: UnsafeMutablePointer<ServerInfo>,
                   callback: ErrorCallback?,
                   callbackData: UnsafeMutableRawPointer) {
    let endpoint = convertServerInfoPtrToEndpoint(serverPtr: serverInfo)

    Task {
        do {
            let service = try await Service.getDefaultService()
            try await service.setDns(endpoint: endpoint)
            callback!(callbackData, nil)
        } catch {
            callback!(callbackData, error.localizedDescription)
        }
    }
}

func createTunInterfaceHandler(_: UnsafeMutableRawPointer,
                               subnet: UnsafeMutablePointer<CChar>,
                               callback: ErrorPayloadCallback_RawDeviceHandle?,
                               callbackData: UnsafeMutableRawPointer) {
    let subnet = String.init(cString: subnet)
    
    Task {
        do {
            let service = try await Service.getDefaultService()
            let fileDescriptor = try await service.createTunInterface(subnet: subnet)
            callback!(callbackData, fileDescriptor.dup(), nil)
        } catch {
            callback!(callbackData, -1, error.localizedDescription)
        }
    }
}

func doneHandler(userdata: UnsafeMutableRawPointer, error: UnsafePointer<CChar>?) {
    let wrappedClosure: WrapClosure<(Result<Void, String>) -> Void> = Unmanaged.fromOpaque(userdata).takeRetainedValue()

    guard let err = error else {
        wrappedClosure.closure(.success(()))
        return
    }

    wrappedClosure.closure(.failure(String.init(cString: err)))
}

private func startServer(configUrl: URL,
                         routeTraffic: Bool,
                         closure: @escaping (Result<Void, String>) -> Void) -> StopHandle {
    let wrappedClosure = WrapClosure(closure: closure)
    let data = Unmanaged.passRetained(wrappedClosure).toOpaque()

    let context = Specht2Context(data: data,
                                 set_http_proxy_handler: setHttpProxyHandler,
                                 set_socks5_proxy_handler: setSocks5ProxyHandler,
                                 set_dns_handler: setDnsHandler,
                                 create_tun_interface_handler: createTunInterfaceHandler,
                                 done_handler: doneHandler)

    return StopHandle(handle: configUrl.withUnsafeFileSystemRepresentation {
        // Due to the limit of cbindgen, the generated signiture for NonNull is non-const,
        // thus the pointer has to be mutable.
        // But we won't mutate it anywhere.
        // The $0 should not be nil according to the docs since the path should always be
        // possible to have a valid representation.
        let mutablePtr = UnsafeMutablePointer(mutating: $0!)
        return specht2_start(mutablePtr, routeTraffic, context)
    })
}

private class WrapClosure<T> {
    let closure: T

    init(closure: T) {
        self.closure = closure
    }
}

func convertServerInfoPtrToEndpoint(serverPtr: UnsafePointer<ServerInfo>) -> Endpoint? {
    guard let addr = serverPtr.pointee.addr else {
        return nil
    }
    return Endpoint(addr: String.init(cString: addr), port: serverPtr.pointee.port)
}
