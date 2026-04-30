use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::path::{Component, Path, PathBuf};

#[cfg(not(any(unix, windows)))]
compile_error!("Only target families 'unix' and 'windows' are supported!");

/// Relative path referring to a directory or (regular) file within a repository. Composed of zero
/// or more [`RepoPathComponent`]s. A path with zero components corresponds to the repository root
/// directory. Imposes some limitations to ensure compatibility with Linux paths:
/// - Combined length of path components plus path separators must be less than or equal to 4096.
///
/// Limitations for path components:
/// - Length must be between 1 and 255 bytes, inclusive.
/// - May not contain the null byte ('\0') or the slash symbol ('/').
/// - May not be equal to '.' or '..'.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RepoPath {
    components: Box<[RepoPathComponent]>,
}

/// Component of a [`RepoPath`] corresponding to a directory or file name.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RepoPathComponent {
    pub inner: Box<[u8]>,
}

/// Error resulting from a failed `RepoPath` conversion.
pub struct RepoPathError;

impl RepoPath {
    pub fn new() -> RepoPath {
        RepoPath {
            components: vec![].into_boxed_slice(),
        }
    }

    pub fn components(&self) -> &[RepoPathComponent] {
        &self.components
    }
}

impl Display for RepoPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.components.iter();
        if let Some(first) = iter.next() {
            write!(f, "{}", String::from_utf8_lossy(&first.inner))?;
            for comp in iter {
                write!(f, "/{}", String::from_utf8_lossy(&comp.inner))?;
            }
        }
        Ok(())
    }
}

impl TryFrom<&RepoPath> for PathBuf {
    type Error = RepoPathError;

    /// Returns [`RepoPathError`] if called on a platform that requires UTF-8 encoded path
    /// components and `value` contains non UTF-8 path components.
    fn try_from(value: &RepoPath) -> Result<PathBuf, Self::Error> {
        let mut path = PathBuf::new();

        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            for comp in &value.components {
                path.push(OsStr::from_bytes(&comp.inner));
            }
        }

        #[cfg(windows)]
        {
            for comp in &value.components {
                let utf8_str = str::from_utf8(&comp.inner).map_err(|_| RepoPathError)?;
                path.push(&OsStr::new(utf8_str));
            }
        }

        Ok(path)
    }
}

impl TryFrom<&Path> for RepoPath {
    type Error = RepoPathError;

    /// Returns [`RepoPathError`] if `value` is an absolute path or breaks any limitations as
    /// specified in the module-level documentation.
    ///
    /// May also error if called on a platform that uses UTF-8 encoded path components and `value`
    /// is malformed.
    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        let mut components = vec![];
        let mut total_len = 0;

        for comp in value.components() {
            match comp {
                Component::Normal(comp) => {
                    #[cfg(unix)]
                    let bytes = {
                        use std::os::unix::ffi::OsStrExt;
                        comp.as_bytes()
                    };

                    #[cfg(windows)]
                    let bytes = comp.to_str().ok_or(RepoPathError)?.as_bytes();

                    let len = bytes.len();
                    // plus path separator before component
                    total_len += len + 1;

                    let has_invalid_chars = bytes.iter().any(|b| *b == b'\0' || *b == b'/');
                    if !(1usize..=255).contains(&bytes.len()) || has_invalid_chars {
                        return Err(RepoPathError);
                    }
                    components.push(RepoPathComponent {
                        inner: bytes.into(),
                    });
                }
                _ => return Err(RepoPathError),
            }
        }

        // minus first path separator
        if total_len > 0 && total_len - 1 > 4096 {
            return Err(RepoPathError);
        }

        Ok(RepoPath {
            components: components.into_boxed_slice(),
        })
    }
}

#[cfg(test)]
mod tests {
    // todo: unit tests
}
