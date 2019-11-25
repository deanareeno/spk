Name: spenv
Version: %(grep -oP '(?<=_version__ = ").*(?=")' spenv/__init__.py)
Release: 1
Summary: Runtime environment management.
License: NONE
Requires: python37
Source0:  %{expand:%%(pwd)}/

%description

%prep
find %{_sourcedir}/ -mindepth 1 -delete
rsync -rav \
    --delete \
    --cvs-exclude \
    --filter=":- .gitignore" \
    %{SOURCEURL0} %{_sourcedir}/

%build
build_dir="$(pwd)"
cd %{_sourcedir}
./build.sh "${build_dir}"

%install
mkdir -p %{buildroot}/usr/local/bin
install -p -m 755 %{_builddir}/bin/spenv %{buildroot}/usr/local/bin/
install -p -m 755 %{_builddir}/bin/spenv-enter %{buildroot}/usr/local/bin/

%files
/usr/local/bin/spenv
%caps(cap_setuid,cap_sys_admin+ep) /usr/local/bin/spenv-enter

%post
mkdir -p /env
chmod 777 /env
