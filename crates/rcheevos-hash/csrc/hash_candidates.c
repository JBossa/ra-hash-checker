/* Compiled in place of rcheevos' src/rhash/hash.c: it includes that file so it can reach the
 * static rc_hash_initialize_iterator_from_path, which populates iterator->consoles without
 * hashing anything (rc_hash_iterate would hash the first candidate as a side effect, and
 * silently falls back to Game Boy for unknown extensions). */
#include "rhash/hash.c"

/* Fills iterator->consoles from the file extension. Returns 0 (leaving consoles empty)
 * if the extension has no handler. */
int rch_populate_candidates(rc_hash_iterator_t* iterator)
{
  size_t num_handlers;
  const rc_hash_iterator_ext_handler_entry_t* handlers = rc_hash_get_iterator_ext_handlers(&num_handlers);
  rc_hash_iterator_ext_handler_entry_t search;
  const char* ext;
  size_t index;

  if (!iterator->path)
    return 0;

  ext = rc_path_get_extension(iterator->path);
  memset(&search, 0, sizeof(search));
  for (index = 0; index < sizeof(search.ext) - 1; ++index) {
    const int c = (int)ext[index];
    if (!c)
      break;
    search.ext[index] = (char)tolower(c);
  }

  if (!bsearch(&search, handlers, num_handlers, sizeof(*handlers), rc_hash_iterator_find_handler))
    return 0;

  rc_hash_initialize_iterator_from_path(iterator, iterator->path);
  return 1;
}
