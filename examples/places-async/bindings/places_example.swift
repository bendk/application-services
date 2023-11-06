import places_async_mod
import Foundation

var counter = DispatchGroup()

class PlacesApiSwift {
    let api = PlacesApi()
    let writerQueue = DispatchQueue(label: "org.mozilla.appservices.places.writer")
    let syncQueue = DispatchQueue(label: "org.mozilla.appservices.places.sync")

    func insertBookmark(bookmark: Bookmark) {
        writerQueue.sync {
            api.insertBookmark(bookmark: bookmark)
        }
    }

    func sync() {
        syncQueue.sync {
            api.sync()
        }
    }
}

let api = PlacesApiSwift()

counter.enter()
Task {
    for i in 0...20 {
        let bookmark = Bookmark(url: "https://example.com/my-bookmarks/\(i)", title: "My Bookmark \(i)")
        api.insertBookmark(bookmark: bookmark)
    }
    counter.leave()
}

counter.enter()
Task {
    api.sync()
    counter.leave()
}

counter.wait()
