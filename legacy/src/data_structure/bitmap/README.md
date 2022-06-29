Many common bitmap functionalities are implemented in [CRoaring](https://github.com/RoaringBitmap/CRoaring).

For small bitmaps (<1k columns), many of the techniques in more sophisticated
algorithms, such as Roaring simply aren't applicable, hence there is still
value in using a simple bitset for such cases.
However, if we ever need sophisticated bitmaps, we should consider linking
against the Roaring library or others instead of implementing our own.
