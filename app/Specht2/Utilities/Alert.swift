//
//  Alert.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/16.
//

import Foundation
import AppKit

class Alert {
    static func alert(message: String) {
        DispatchQueue.main.async {
            let alert = NSAlert()
            alert.messageText = message
            alert.runModal()
        }
    }
}
