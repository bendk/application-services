import kotlinx.coroutines.*
import uniffi.places.Bookmark
import uniffi.places.PlacesApi

class PlacesApiKotlin {
    val api = PlacesApi(Dispatchers.IO)

    suspend fun insertBookmark(bookmark: Bookmark) {
        api.insertBookmark(bookmark)
    }

    suspend fun sync() {
        api.sync()
    }
}

runBlocking(Dispatchers.Default) {
    coroutineScope {
        val api = PlacesApiKotlin()
        launch {
            for (i in 0..20) {
                val bookmark = Bookmark(
                    url="https://example.com/my-bookmarks/${i}",
                    title="My Bookmark ${i}"
                )
                api.insertBookmark(bookmark)
            }
        }
        launch {
            api.sync()
        }
    }
}
