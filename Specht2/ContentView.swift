//
//  ContentView.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/11/11.
//

import SwiftUI

struct ContentView: View {
    var body: some View {
        Button {
            Task {
                try! await Manager.createManager()
            }
        } label: {
            Text("Create VPN")
        }
        .padding()
    }
}

struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
    }
}
