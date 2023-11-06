import places_async_mod
import Foundation

var counter = DispatchGroup()

class PlacesApiSwift {
    let api = PlacesApi(queue: DispatchQueue.global(qos: .userInitiated))

    func insertBookmark(bookmark: Bookmark) async {
        await api.insertBookmark(bookmark: bookmark)
    }

    func sync() async {
        await api.sync()
    }
}

let api = PlacesApiSwift()

counter.enter()
Task {
    for i in 0...20 {
        let bookmark = Bookmark(url: "https://example.com/my-bookmarks/\(i)", title: "My Bookmark \(i)")
        await api.insertBookmark(bookmark: bookmark)
    }
    counter.leave()
}

counter.enter()
Task {
    await api.sync()
    counter.leave()
}

counter.wait()
